use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::SystemTime;

use flate2::read::GzDecoder;

use crate::cache::ScanCache;
use crate::models::{
    AgentStats, CacheEfficiency, DashboardSummary, DayAccum, DayRow, DayTokenRow,
    DurationBucket, ErrorRate, EventFile, FileScanStats, HookAccum, HookPerfRow, RecentEvent,
    RecentEventAccum, SessionAccum, SessionRow, SessionTimelineRow, TokenUsage,
    TokenUsageSummary, ToolUsageRow, Totals, TurnDurationSummary, TypeBuckets,
};
use crate::parse::{
    classify_file_type, epoch_millis_to_iso, looks_like_day, now_iso, parse_event_line,
    system_time_to_iso, system_time_to_millis,
};

pub async fn build_summary(
    root: &Path,
    cache: Option<&ScanCache>,
) -> Result<DashboardSummary, Box<dyn std::error::Error>> {
    let mut totals = Totals::default();
    let mut newest_file_mtime: Option<SystemTime> = None;

    let mut sessions: HashMap<String, SessionAccum> = HashMap::new();
    let mut days: BTreeMap<String, DayAccum> = BTreeMap::new();
    let mut types = TypeBuckets::default();
    let mut recent_events: Vec<RecentEventAccum> = Vec::new();
    let mut agent_stats: HashMap<String, AgentStats> = HashMap::new();

    let mut global_tokens = TokenUsage::default();
    let mut global_tool_counts: HashMap<String, u64> = HashMap::new();
    let mut global_turn_durations: Vec<i64> = Vec::new();
    let mut global_hook_stats: HashMap<String, HookAccum> = HashMap::new();
    let mut global_api_errors: u64 = 0;
    let mut global_tool_errors: u64 = 0;
    let mut session_hourly: HashMap<String, [u64; 24]> = HashMap::new();

    let event_files = iter_event_files(root)?;

    for event_file in &event_files {
        let metadata = fs::metadata(&event_file.path)?;
        let size_bytes = metadata.len();
        let mtime = metadata.modified().ok();
        let mtime_ms = mtime.map(system_time_to_millis).unwrap_or(0);
        let path_str = event_file.path.to_string_lossy().to_string();

        let cached = match cache {
            Some(c) => c.lookup(&path_str, mtime_ms, size_bytes).await,
            None => None,
        };
        let scan_stats = match cached {
            Some(stats) => stats,
            None => {
                let stats = count_lines_and_collect_recent(event_file)?;
                if let Some(c) = cache {
                    c.store(&path_str, mtime_ms, size_bytes, &stats).await;
                }
                stats
            }
        };

        let event_count = scan_stats.event_count;

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

        // Extract new per-file stats before consuming session_stats
        day_entry.tokens += scan_stats.token_usage.clone();
        global_tokens += scan_stats.token_usage;
        for (tool, count) in scan_stats.tool_counts {
            *global_tool_counts.entry(tool).or_insert(0) += count;
        }
        global_turn_durations.extend(scan_stats.turn_durations);
        for (command, accum) in scan_stats.hook_stats {
            let entry = global_hook_stats.entry(command).or_default();
            entry.count += accum.count;
            entry.total_ms += accum.total_ms;
        }
        global_api_errors += scan_stats.api_error_count;
        global_tool_errors += scan_stats.tool_error_count;

        let mut file_agents: HashSet<String> = HashSet::new();

        for (session_id, session_stats) in scan_stats.session_stats {
            day_entry.sessions.insert(session_id.clone());

            // Merge session hourly events
            let hourly = session_hourly.entry(session_id.clone()).or_insert([0u64; 24]);
            for (h, val) in hourly.iter_mut().enumerate() {
                *val += session_stats.hourly_events[h];
            }

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

        for event in scan_stats.recent_events {
            insert_recent_event(&mut recent_events, event);
        }
    }

    if let Some(c) = cache {
        let valid: Vec<String> = event_files
            .iter()
            .map(|f| f.path.to_string_lossy().to_string())
            .collect();
        c.remove_stale(&valid).await;
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

    let mut day_rows: Vec<DayRow> = Vec::new();
    let mut day_token_rows: Vec<DayTokenRow> = Vec::new();
    for (day, rec) in days {
        day_token_rows.push(DayTokenRow {
            day: day.clone(),
            input_tokens: rec.tokens.input_tokens,
            output_tokens: rec.tokens.output_tokens,
            cache_read_tokens: rec.tokens.cache_read_input_tokens,
            cache_creation_tokens: rec.tokens.cache_creation_input_tokens,
        });
        day_rows.push(DayRow {
            day,
            events: rec.events,
            bytes: rec.bytes,
            files: rec.files,
            sessions: rec.sessions.len() as u64,
        });
    }
    day_rows.sort_by(|a, b| b.day.cmp(&a.day));
    day_token_rows.sort_by(|a, b| b.day.cmp(&a.day));

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

    // Build token usage summary
    let token_usage = TokenUsageSummary {
        total_input: global_tokens.input_tokens,
        total_output: global_tokens.output_tokens,
        total_cache_read: global_tokens.cache_read_input_tokens,
        total_cache_creation: global_tokens.cache_creation_input_tokens,
        daily: day_token_rows,
    };

    // Build tool usage rows (sorted by count desc, top 20)
    let mut tool_usage: Vec<ToolUsageRow> = global_tool_counts
        .into_iter()
        .map(|(tool, count)| ToolUsageRow { tool, count })
        .collect();
    tool_usage.sort_by(|a, b| b.count.cmp(&a.count));
    tool_usage.truncate(20);

    // Build turn duration summary with 5 fixed buckets
    let turn_durations = build_turn_duration_summary(&global_turn_durations);

    // Build session timeline (top 8 sessions by event count)
    let mut session_timeline_entries: Vec<(String, u64, [u64; 24])> = session_hourly
        .into_iter()
        .map(|(session, hours)| {
            let total: u64 = hours.iter().sum();
            (session, total, hours)
        })
        .collect();
    session_timeline_entries.sort_by(|a, b| b.1.cmp(&a.1));
    session_timeline_entries.truncate(8);
    let session_timeline: Vec<SessionTimelineRow> = session_timeline_entries
        .into_iter()
        .map(|(session, _, hours)| SessionTimelineRow {
            session,
            hours: hours.to_vec(),
        })
        .collect();

    // Build cache efficiency
    let cache_total = global_tokens.cache_read_input_tokens + global_tokens.cache_creation_input_tokens;
    let cache_efficiency = CacheEfficiency {
        total_cache_read: global_tokens.cache_read_input_tokens,
        total_cache_creation: global_tokens.cache_creation_input_tokens,
        ratio: if cache_total > 0 {
            global_tokens.cache_read_input_tokens as f64 / cache_total as f64
        } else {
            0.0
        },
    };

    // Build error rate
    let total_events = totals.events;
    let error_rate = ErrorRate {
        api_errors: global_api_errors,
        tool_errors: global_tool_errors,
        total_events,
        api_error_rate: if total_events > 0 {
            global_api_errors as f64 / total_events as f64
        } else {
            0.0
        },
        tool_error_rate: if total_events > 0 {
            global_tool_errors as f64 / total_events as f64
        } else {
            0.0
        },
    };

    // Build hook performance rows (sorted by count desc)
    let mut hook_performance: Vec<HookPerfRow> = global_hook_stats
        .into_iter()
        .map(|(command, accum)| HookPerfRow {
            command,
            count: accum.count,
            avg_ms: if accum.count > 0 {
                accum.total_ms / accum.count as i64
            } else {
                0
            },
            total_ms: accum.total_ms,
        })
        .collect();
    hook_performance.sort_by(|a, b| b.count.cmp(&a.count));

    Ok(DashboardSummary {
        generated_at: now_iso(),
        root: root.display().to_string(),
        totals,
        types,
        days: day_rows,
        top_sessions: session_rows,
        recent_events: recent_event_rows,
        agents: agent_stats,
        token_usage,
        tool_usage,
        turn_durations,
        session_timeline,
        cache_efficiency,
        error_rate,
        hook_performance,
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

            // Flat layout: day/session/events.jsonl (no channel dir)
            for file_name in ["events.jsonl", "events.jsonl.gz"] {
                let candidate = session_path.join(file_name);
                if candidate.is_file() {
                    files.push(EventFile {
                        day: day_name.clone(),
                        session: session_name.clone(),
                        channel: String::new(),
                        file_type: "json".to_string(),
                        path: candidate,
                    });
                }
            }

            // Nested layout: day/session/channel/events.jsonl
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
) -> Result<FileScanStats, Box<dyn std::error::Error>> {
    let file = fs::File::open(&event_file.path)?;
    if event_file.path.extension().and_then(|v| v.to_str()) == Some("gz") {
        let decoder = GzDecoder::new(file);
        let reader = BufReader::new(decoder);
        return scan_event_lines(reader, event_file);
    }

    let reader = BufReader::new(file);
    scan_event_lines(reader, event_file)
}

pub fn scan_event_lines<R: BufRead>(
    reader: R,
    event_file: &EventFile,
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
                // Extract hour for session timeline
                let hour = ((last_seen_ms / 1000) % 86400 / 3600) as usize;
                if hour < 24 {
                    session_entry.hourly_events[hour] += 1;
                }
            }

            // Token usage
            if let Some(ref tu) = parsed_event.token_usage {
                stats.token_usage += tu.clone();
            }

            // Tool uses
            for tool_name in &parsed_event.tool_uses {
                *stats.tool_counts.entry(tool_name.clone()).or_insert(0) += 1;
            }

            // Turn durations
            if let Some(dur) = parsed_event.turn_duration_ms {
                stats.turn_durations.push(dur);
            }

            // Hook infos
            for hi in &parsed_event.hook_infos {
                let entry = stats.hook_stats.entry(hi.command.clone()).or_default();
                entry.count += 1;
                entry.total_ms += hi.duration_ms;
            }

            // Errors
            if parsed_event.is_api_error {
                stats.api_error_count += 1;
            }
            if parsed_event.is_tool_error {
                stats.tool_error_count += 1;
            }

            // Model counts
            if let Some(ref model) = parsed_event.model {
                *stats.model_counts.entry(model.clone()).or_insert(0) += 1;
            }

            if let Some(event) = parsed_event.to_recent_event() {
                insert_recent_event(&mut stats.recent_events, event);
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

fn build_turn_duration_summary(durations: &[i64]) -> TurnDurationSummary {
    if durations.is_empty() {
        return TurnDurationSummary::default();
    }

    let min_ms = *durations.iter().min().unwrap_or(&0);
    let max_ms = *durations.iter().max().unwrap_or(&0);
    let sum: i64 = durations.iter().sum();
    let avg_ms = sum / durations.len() as i64;
    let count = durations.len() as u64;

    // 5 fixed buckets: 0-30s, 30-60s, 60-120s, 120-300s, 300s+
    let boundaries = [(0, 30_000, "0-30s"), (30_000, 60_000, "30-60s"), (60_000, 120_000, "1-2m"), (120_000, 300_000, "2-5m"), (300_000, i64::MAX, "5m+")];
    let buckets: Vec<DurationBucket> = boundaries
        .iter()
        .map(|(lo, hi, label)| {
            let c = durations.iter().filter(|d| **d >= *lo && **d < *hi).count() as u64;
            DurationBucket {
                label: label.to_string(),
                count: c,
            }
        })
        .collect();

    TurnDurationSummary {
        buckets,
        min_ms,
        max_ms,
        avg_ms,
        count,
    }
}

pub fn insert_recent_event(recent_pool: &mut Vec<RecentEventAccum>, event: RecentEventAccum) {
    recent_pool.push(event);
    recent_pool.sort_by(|a, b| b.sort_key_ms.cmp(&a.sort_key_ms));
    if recent_pool.len() > 5 {
        recent_pool.truncate(5);
    }
}

