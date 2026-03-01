use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::SystemTime;

use flate2::read::GzDecoder;

use crate::models::{
    AgentStats, DashboardSummary, DayAccum, DayRow, EventFile, FileScanStats, RecentEvent,
    RecentEventAccum, SessionAccum, SessionRow, Totals, TypeBuckets,
};
use crate::parse::{
    classify_file_type, epoch_millis_to_iso, looks_like_day, now_iso, parse_event_line,
    system_time_to_iso, system_time_to_millis,
};

pub fn build_summary(root: &Path) -> Result<DashboardSummary, Box<dyn std::error::Error>> {
    let mut totals = Totals::default();
    let mut newest_file_mtime: Option<SystemTime> = None;

    let mut sessions: HashMap<String, SessionAccum> = HashMap::new();
    let mut days: BTreeMap<String, DayAccum> = BTreeMap::new();
    let mut types = TypeBuckets::default();
    let mut recent_events: Vec<RecentEventAccum> = Vec::new();
    let mut agent_stats: HashMap<String, AgentStats> = HashMap::new();

    for event_file in iter_event_files(root)? {
        let scan_stats = count_lines_and_collect_recent(&event_file, &mut recent_events)?;
        let event_count = scan_stats.event_count;
        let metadata = fs::metadata(&event_file.path)?;
        let size_bytes = metadata.len();
        let mtime = metadata.modified().ok();

        totals.files += 1;
        totals.events += event_count;
        totals.bytes += size_bytes;

        if let Some(m) = mtime {
            newest_file_mtime = Some(match newest_file_mtime {
                Some(current) => std::cmp::max(current, m),
                None => m,
            });
        }

        let day_entry = days.entry(event_file.day.clone()).or_default();
        day_entry.events += event_count;
        day_entry.bytes += size_bytes;
        day_entry.files += 1;

        let mut file_agents: HashSet<String> = HashSet::new();

        for (session_id, session_stats) in scan_stats.session_stats {
            day_entry.sessions.insert(session_id.clone());

            let agent_name = session_stats
                .agent
                .clone()
                .unwrap_or_else(|| "unknown".to_string());
            file_agents.insert(agent_name.clone());

            let agent_entry = agent_stats.entry(agent_name).or_default();
            agent_entry.events += session_stats.events;
            agent_entry.bytes += session_stats.bytes;

            let session_entry = sessions.entry(session_id).or_default();
            session_entry.events += session_stats.events;
            session_entry.bytes += session_stats.bytes;
            session_entry.files += 1;
            session_entry.days.insert(event_file.day.clone());
            if session_entry.agent.is_none() {
                session_entry.agent = session_stats.agent;
            }

            let file_mtime_ms = mtime.map(system_time_to_millis);
            let candidate_last_seen = session_stats.last_seen_ms.or(file_mtime_ms);
            if let Some(last_seen_ms) = candidate_last_seen {
                session_entry.last_seen_ms = Some(match session_entry.last_seen_ms {
                    Some(existing) => existing.max(last_seen_ms),
                    None => last_seen_ms,
                });
            }
        }

        for agent_name in file_agents {
            agent_stats.entry(agent_name).or_default().files += 1;
        }

        let bucket = if event_file.file_type == "json" {
            &mut types.json
        } else {
            &mut types.text
        };
        bucket.events += event_count;
        bucket.bytes += size_bytes;
        bucket.files += 1;
    }

    totals.days = days.len() as u64;
    totals.sessions = sessions.len() as u64;
    totals.newest_file_mtime = newest_file_mtime.map(system_time_to_iso);

    for session_accum in sessions.values() {
        let agent_name = session_accum
            .agent
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        agent_stats.entry(agent_name).or_default().sessions += 1;
    }

    let mut day_rows: Vec<DayRow> = days
        .into_iter()
        .map(|(day, rec)| DayRow {
            day,
            events: rec.events,
            bytes: rec.bytes,
            files: rec.files,
            sessions: rec.sessions.len() as u64,
        })
        .collect();
    day_rows.sort_by(|a, b| b.day.cmp(&a.day));

    let mut session_rows: Vec<SessionRow> = sessions
        .into_iter()
        .map(|(session, rec)| SessionRow {
            session,
            agent: rec.agent.unwrap_or_else(|| "unknown".to_string()),
            events: rec.events,
            bytes: rec.bytes,
            files: rec.files,
            days: rec.days.len() as u64,
            last_seen: rec
                .last_seen_ms
                .map(epoch_millis_to_iso)
                .unwrap_or_else(|| "n/a".to_string()),
        })
        .collect();
    session_rows.sort_by(|a, b| b.events.cmp(&a.events).then(a.session.cmp(&b.session)));
    session_rows.truncate(20);

    recent_events.sort_by(|a, b| b.sort_key_ms.cmp(&a.sort_key_ms));
    recent_events.truncate(5);
    let recent_event_rows: Vec<RecentEvent> =
        recent_events.into_iter().map(|item| item.event).collect();

    Ok(DashboardSummary {
        generated_at: now_iso(),
        root: root.display().to_string(),
        totals,
        types,
        days: day_rows,
        top_sessions: session_rows,
        recent_events: recent_event_rows,
        agents: agent_stats,
    })
}

pub fn iter_event_files(root: &Path) -> Result<Vec<EventFile>, Box<dyn std::error::Error>> {
    let mut files = Vec::new();
    if !root.exists() {
        return Ok(files);
    }

    for day_entry in fs::read_dir(root)? {
        let day_entry = day_entry?;
        if !day_entry.file_type()?.is_dir() {
            continue;
        }
        let day_name = day_entry.file_name().to_string_lossy().to_string();
        if !looks_like_day(&day_name) {
            continue;
        }

        for session_entry in fs::read_dir(day_entry.path())? {
            let session_entry = session_entry?;
            if !session_entry.file_type()?.is_dir() {
                continue;
            }
            let session_name = session_entry.file_name().to_string_lossy().to_string();
            let session_path = session_entry.path();

            for channel_entry in fs::read_dir(&session_path)? {
                let channel_entry = channel_entry?;
                if !channel_entry.file_type()?.is_dir() {
                    continue;
                }
                let channel_name = channel_entry.file_name().to_string_lossy().to_string();
                let file_type = classify_file_type(&channel_name);
                let channel_path = channel_entry.path();

                for file_name in ["events.jsonl", "events.jsonl.gz"] {
                    let candidate = channel_path.join(file_name);
                    if candidate.is_file() {
                        files.push(EventFile {
                            day: day_name.clone(),
                            session: session_name.clone(),
                            channel: channel_name.clone(),
                            file_type: file_type.clone(),
                            path: candidate,
                        });
                    }
                }
            }
        }
    }

    Ok(files)
}

pub fn count_lines_and_collect_recent(
    event_file: &EventFile,
    recent_pool: &mut Vec<RecentEventAccum>,
) -> Result<FileScanStats, Box<dyn std::error::Error>> {
    let file = fs::File::open(&event_file.path)?;
    if event_file.path.extension().and_then(|v| v.to_str()) == Some("gz") {
        let decoder = GzDecoder::new(file);
        let reader = BufReader::new(decoder);
        return scan_event_lines(reader, event_file, recent_pool);
    }

    let reader = BufReader::new(file);
    scan_event_lines(reader, event_file, recent_pool)
}

pub fn scan_event_lines<R: BufRead>(
    reader: R,
    event_file: &EventFile,
    recent_pool: &mut Vec<RecentEventAccum>,
) -> Result<FileScanStats, Box<dyn std::error::Error>> {
    let mut stats = FileScanStats::default();
    for line in reader.lines() {
        let line = line?;
        stats.event_count += 1;

        if let Some(parsed_event) = parse_event_line(&line, event_file) {
            let session_entry = stats
                .session_stats
                .entry(parsed_event.session.clone())
                .or_default();
            session_entry.events += 1;
            session_entry.bytes += line.len() as u64;
            if session_entry.agent.is_none() && parsed_event.agent != "unknown" {
                session_entry.agent = Some(parsed_event.agent.clone());
            }
            if let Some(last_seen_ms) = parsed_event.timestamp_ms {
                session_entry.last_seen_ms = Some(match session_entry.last_seen_ms {
                    Some(existing) => existing.max(last_seen_ms),
                    None => last_seen_ms,
                });
            }

            if let Some(event) = parsed_event.to_recent_event() {
                insert_recent_event(recent_pool, event);
            }
        } else {
            let fallback_entry = stats
                .session_stats
                .entry(event_file.session.clone())
                .or_default();
            fallback_entry.events += 1;
            fallback_entry.bytes += line.len() as u64;
        }
    }
    Ok(stats)
}

pub fn insert_recent_event(recent_pool: &mut Vec<RecentEventAccum>, event: RecentEventAccum) {
    recent_pool.push(event);
    recent_pool.sort_by(|a, b| b.sort_key_ms.cmp(&a.sort_key_ms));
    if recent_pool.len() > 5 {
        recent_pool.truncate(5);
    }
}

