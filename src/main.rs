use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Instant, SystemTime};

use axum::extract::State;
use axum::response::Html;
use axum::routing::get;
use axum::{Json, Router};
use chrono::{DateTime, TimeZone, Utc};
use clap::Parser;
use flate2::read::GzDecoder;
use serde::Serialize;
use tokio::sync::Mutex;

#[derive(Parser, Debug, Clone)]
#[command(name = "peeprs")]
#[command(about = "Claude centralized logs dashboard")]
struct Args {
    #[arg(long, default_value = "~/logs/claude")]
    root: String,

    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    #[arg(long, default_value_t = 8765)]
    port: u16,

    #[arg(long, default_value_t = 10)]
    refresh_seconds: u64,

    #[arg(long, default_value_t = 5)]
    cache_seconds: u64,
}

#[derive(Debug, Clone)]
struct AppState {
    root: PathBuf,
    refresh_ms: u64,
    cache_seconds: u64,
    cache: Arc<Mutex<SummaryCache>>,
}

#[derive(Debug, Default)]
struct SummaryCache {
    at: Option<Instant>,
    summary: Option<DashboardSummary>,
}

#[derive(Debug, Clone, Serialize)]
struct DashboardSummary {
    generated_at: String,
    root: String,
    totals: Totals,
    types: TypeBuckets,
    days: Vec<DayRow>,
    top_sessions: Vec<SessionRow>,
    recent_events: Vec<RecentEvent>,
}

#[derive(Debug, Clone, Serialize, Default)]
struct Totals {
    days: u64,
    sessions: u64,
    files: u64,
    events: u64,
    bytes: u64,
    newest_file_mtime: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
struct TypeBuckets {
    json: TypeStats,
    text: TypeStats,
}

#[derive(Debug, Clone, Serialize, Default)]
struct TypeStats {
    events: u64,
    bytes: u64,
    files: u64,
}

#[derive(Debug, Clone, Serialize)]
struct DayRow {
    day: String,
    events: u64,
    bytes: u64,
    files: u64,
    sessions: u64,
}

#[derive(Debug, Clone, Serialize)]
struct SessionRow {
    session: String,
    events: u64,
    bytes: u64,
    files: u64,
    days: u64,
    last_seen: String,
}

#[derive(Debug, Clone, Serialize)]
struct RecentEvent {
    timestamp: String,
    session: String,
    event_type: String,
    preview: String,
}

#[derive(Debug, Clone)]
struct RecentEventAccum {
    sort_key_ms: i64,
    event: RecentEvent,
}

#[derive(Debug, Default)]
struct FileScanStats {
    event_count: u64,
    session_stats: HashMap<String, SessionLineStats>,
}

#[derive(Debug, Default)]
struct SessionLineStats {
    events: u64,
    bytes: u64,
    last_seen_ms: Option<i64>,
}

#[derive(Debug, Clone)]
struct ParsedEventLine {
    session: String,
    event_type: String,
    preview: String,
    timestamp_ms: Option<i64>,
    timestamp_display: Option<String>,
}

impl ParsedEventLine {
    fn to_recent_event(&self) -> Option<RecentEventAccum> {
        Some(RecentEventAccum {
            sort_key_ms: self.timestamp_ms?,
            event: RecentEvent {
                timestamp: self.timestamp_display.clone()?,
                session: self.session.clone(),
                event_type: self.event_type.clone(),
                preview: self.preview.clone(),
            },
        })
    }
}

#[derive(Debug, Default)]
struct SessionAccum {
    events: u64,
    bytes: u64,
    files: u64,
    days: HashSet<String>,
    last_seen_ms: Option<i64>,
}

#[derive(Debug, Default)]
struct DayAccum {
    events: u64,
    bytes: u64,
    files: u64,
    sessions: HashSet<String>,
}

#[derive(Debug)]
struct EventFile {
    day: String,
    session: String,
    channel: String,
    file_type: String,
    path: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let root = expand_tilde(&args.root);
    let host = args.host.clone();
    let port = args.port;

    let state = AppState {
        root,
        refresh_ms: args.refresh_seconds.max(1) * 1000,
        cache_seconds: args.cache_seconds.max(1),
        cache: Arc::new(Mutex::new(SummaryCache::default())),
    };

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/index.html", get(index_handler))
        .route("/api/summary", get(summary_handler))
        .route("/healthz", get(health_handler))
        .with_state(state.clone());

    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;

    println!("Dashboard listening on http://{}:{}", host, port);
    println!("Log root: {}", state.root.display());

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn index_handler(State(state): State<AppState>) -> Html<String> {
    Html(render_html(state.refresh_ms))
}

async fn summary_handler(
    State(state): State<AppState>,
) -> Result<Json<DashboardSummary>, (axum::http::StatusCode, String)> {
    let mut cache = state.cache.lock().await;
    let is_fresh = cache
        .at
        .map(|at| at.elapsed().as_secs() < state.cache_seconds)
        .unwrap_or(false);

    if is_fresh {
        if let Some(summary) = &cache.summary {
            return Ok(Json(summary.clone()));
        }
    }

    let summary = build_summary(&state.root).map_err(|err| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to build summary: {err}"),
        )
    })?;

    cache.at = Some(Instant::now());
    cache.summary = Some(summary.clone());
    Ok(Json(summary))
}

async fn health_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "ok": true, "time": now_iso() }))
}

fn build_summary(root: &Path) -> Result<DashboardSummary, Box<dyn std::error::Error>> {
    let mut totals = Totals::default();
    let mut newest_file_mtime: Option<SystemTime> = None;

    let mut sessions: HashMap<String, SessionAccum> = HashMap::new();
    let mut days: BTreeMap<String, DayAccum> = BTreeMap::new();
    let mut types = TypeBuckets::default();
    let mut recent_events: Vec<RecentEventAccum> = Vec::new();

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

        for (session_id, session_stats) in scan_stats.session_stats {
            day_entry.sessions.insert(session_id.clone());

            let session_entry = sessions.entry(session_id).or_default();
            session_entry.events += session_stats.events;
            session_entry.bytes += session_stats.bytes;
            session_entry.files += 1;
            session_entry.days.insert(event_file.day.clone());

            let file_mtime_ms = mtime.map(system_time_to_millis);
            let candidate_last_seen = session_stats.last_seen_ms.or(file_mtime_ms);
            if let Some(last_seen_ms) = candidate_last_seen {
                session_entry.last_seen_ms = Some(match session_entry.last_seen_ms {
                    Some(existing) => existing.max(last_seen_ms),
                    None => last_seen_ms,
                });
            }
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
    })
}

fn iter_event_files(root: &Path) -> Result<Vec<EventFile>, Box<dyn std::error::Error>> {
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

fn looks_like_day(value: &str) -> bool {
    if value.len() != 10 {
        return false;
    }
    let bytes = value.as_bytes();
    bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(idx, b)| idx == 4 || idx == 7 || b.is_ascii_digit())
}

fn classify_file_type(channel_name: &str) -> String {
    if channel_name.eq_ignore_ascii_case("text") {
        "text".to_string()
    } else {
        "json".to_string()
    }
}

fn count_lines_and_collect_recent(
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

fn scan_event_lines<R: BufRead>(
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

fn parse_event_line(line: &str, event_file: &EventFile) -> Option<ParsedEventLine> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    let timestamp = extract_event_timestamp(&value);
    let (timestamp_ms, timestamp_display) = match timestamp {
        Some((ms, display)) => (Some(ms), Some(display)),
        None => (None, None),
    };
    let session = extract_event_session(&value, &event_file.session);
    let event_type = extract_event_type(&value, &event_file.channel);
    let preview = extract_event_preview(&value);

    Some(ParsedEventLine {
        session,
        event_type,
        preview,
        timestamp_ms,
        timestamp_display,
    })
}

fn insert_recent_event(recent_pool: &mut Vec<RecentEventAccum>, event: RecentEventAccum) {
    recent_pool.push(event);
    recent_pool.sort_by(|a, b| b.sort_key_ms.cmp(&a.sort_key_ms));
    if recent_pool.len() > 5 {
        recent_pool.truncate(5);
    }
}

fn extract_event_timestamp(value: &serde_json::Value) -> Option<(i64, String)> {
    for key in ["timestamp", "ingest_ts"] {
        if let Some(raw) = value.get(key) {
            if let Some((sort_key_ms, display)) = parse_timestamp_value(raw) {
                return Some((sort_key_ms, display));
            }
        }
    }
    None
}

fn parse_timestamp_value(value: &serde_json::Value) -> Option<(i64, String)> {
    if let Some(ts) = value.as_str() {
        if let Ok(parsed) = DateTime::parse_from_rfc3339(ts) {
            let utc = parsed.with_timezone(&Utc);
            return Some((
                utc.timestamp_millis(),
                utc.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            ));
        }

        if let Ok(num) = ts.parse::<i64>() {
            let ms = normalize_epoch_millis(num);
            return Some((ms, epoch_millis_to_iso(ms)));
        }
    }

    if let Some(num) = value.as_i64() {
        let ms = normalize_epoch_millis(num);
        return Some((ms, epoch_millis_to_iso(ms)));
    }

    None
}

fn normalize_epoch_millis(raw: i64) -> i64 {
    if raw > 100_000_000_000 {
        raw
    } else {
        raw.saturating_mul(1000)
    }
}

fn epoch_millis_to_iso(ms: i64) -> String {
    Utc.timestamp_millis_opt(ms)
        .single()
        .map(|ts| ts.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
        .unwrap_or_else(|| "n/a".to_string())
}

fn system_time_to_millis(value: SystemTime) -> i64 {
    DateTime::<Utc>::from(value).timestamp_millis()
}

fn extract_event_session(value: &serde_json::Value, fallback_session: &str) -> String {
    value
        .get("session")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty() && *value != "unknown")
        .or_else(|| {
            value
                .get("sessionId")
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.is_empty())
        })
        .unwrap_or(fallback_session)
        .to_string()
}

fn extract_event_type(value: &serde_json::Value, fallback_channel: &str) -> String {
    value
        .get("type")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback_channel)
        .to_string()
}

fn extract_event_preview(value: &serde_json::Value) -> String {
    let preview = value
        .get("display")
        .and_then(serde_json::Value::as_str)
        .or_else(|| value.get("message").and_then(serde_json::Value::as_str))
        .or_else(|| {
            value
                .get("message")
                .and_then(|message| message.get("content"))
                .and_then(serde_json::Value::as_str)
        })
        .or_else(|| {
            value
                .get("message")
                .and_then(|message| message.get("content"))
                .and_then(serde_json::Value::as_array)
                .and_then(|items| {
                    items.iter().find_map(|item| {
                        item.get("text")
                            .and_then(serde_json::Value::as_str)
                            .or_else(|| item.get("content").and_then(serde_json::Value::as_str))
                    })
                })
        })
        .or_else(|| value.get("text").and_then(serde_json::Value::as_str))
        .unwrap_or("(event)");

    compact_preview(preview, 110)
}

fn compact_preview(value: &str, max_chars: usize) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.is_empty() {
        return "(event)".to_string();
    }
    if compact.chars().count() <= max_chars {
        return compact;
    }

    let short = compact
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    format!("{}...", short)
}

fn system_time_to_iso(value: SystemTime) -> String {
    DateTime::<Utc>::from(value).to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn expand_tilde(input: &str) -> PathBuf {
    if let Some(stripped) = input.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(stripped);
        }
    }
    PathBuf::from(input)
}

fn render_html(refresh_ms: u64) -> String {
    let template = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <title>Claude Logs Dashboard</title>
  <style>
    :root {
      --bg-a: #0b1320;
      --bg-b: #0f1a2b;
      --bg-c: #0a121f;
      --card: #111b2b;
      --card-strong: #0c1624;
      --ink: #eaf1fc;
      --muted: #9fb1c8;
      --line: #203047;
      --line-soft: #1a273c;
      --accent: #4da3ff;
      --accent-soft: #1a2c45;
      --json: #40b8ff;
      --text: #f9b266;
      --good: #4bc08c;
      --shadow: 0 8px 18px rgba(1, 8, 18, 0.24);
    }

    * { box-sizing: border-box; }

    body {
      margin: 0;
      font-family: "Plus Jakarta Sans", "IBM Plex Sans", "Avenir Next", "Segoe UI", sans-serif;
      color: var(--ink);
      background:
        radial-gradient(1000px 620px at -10% -18%, #17335d 0%, transparent 58%),
        radial-gradient(1000px 620px at 112% 118%, #1b3552 0%, transparent 56%),
        linear-gradient(150deg, var(--bg-a), var(--bg-b) 54%, var(--bg-c));
      min-height: 100vh;
      padding: 14px;
    }

    body::before {
      content: "";
      position: fixed;
      inset: 0;
      pointer-events: none;
      background-image:
        linear-gradient(rgba(120, 150, 190, 0.06) 1px, transparent 1px),
        linear-gradient(90deg, rgba(120, 150, 190, 0.06) 1px, transparent 1px);
      background-size: 28px 28px;
      opacity: 0.16;
      mask-image: radial-gradient(circle at 30% 20%, black 20%, transparent 75%);
    }

    .wrap { max-width: 1480px; margin: 0 auto; }

    .header {
      display: flex;
      align-items: end;
      justify-content: space-between;
      gap: 12px;
      margin-bottom: 10px;
    }

    h1 {
      margin: 0;
      font-size: 27px;
      letter-spacing: 0.16px;
      font-weight: 700;
    }

    .sub {
      color: var(--muted);
      margin-top: 3px;
      font-size: 12px;
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
    }

    .kpi-grid {
      display: grid;
      grid-template-columns: repeat(10, minmax(0, 1fr));
      gap: 8px;
      margin-bottom: 10px;
    }

    @media (max-width: 1240px) {
      .kpi-grid { grid-template-columns: repeat(5, minmax(0, 1fr)); }
    }

    @media (max-width: 740px) {
      .kpi-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
      .header { flex-direction: column; align-items: start; }
    }

    .card {
      background: linear-gradient(180deg, #142238 0%, var(--card) 100%);
      border: 1px solid var(--line);
      border-radius: 8px;
      padding: 10px;
      box-shadow: var(--shadow);
      animation: card-enter 180ms ease both;
    }

    .kpi {
      min-height: 72px;
      border-color: var(--line-soft);
    }

    .k {
      color: var(--muted);
      font-size: 10px;
      text-transform: uppercase;
      letter-spacing: 0.58px;
      font-weight: 600;
    }

    .v {
      font-size: 24px;
      font-weight: 700;
      margin-top: 4px;
      line-height: 1.1;
    }

    .v-small {
      font-size: 16px;
      margin-top: 7px;
      color: #d5e4f9;
      font-weight: 650;
    }

    .main {
      display: grid;
      grid-template-columns: 2fr 1fr;
      gap: 8px;
      margin-bottom: 8px;
    }

    @media (max-width: 1020px) {
      .main { grid-template-columns: 1fr; }
    }

    h2 {
      margin: 1px 0 8px 0;
      font-size: 13px;
      letter-spacing: 0.22px;
      font-weight: 680;
    }

    .panel {
      display: grid;
      gap: 8px;
    }

    .chart-card {
      height: 220px;
      display: grid;
      grid-template-rows: auto 1fr;
    }

    .bars {
      display: grid;
      align-items: end;
      height: 170px;
      gap: 2px;
      grid-template-columns: repeat(21, minmax(0, 1fr));
      padding-top: 6px;
    }

    .bar-col {
      border-radius: 3px 3px 0 0;
      background: linear-gradient(180deg, #4ca8ff 0%, #2f71c5 100%);
      min-height: 2px;
      opacity: 0.92;
      transition: opacity 140ms ease;
    }

    .bar-col:hover { opacity: 1; }

    .chart-meta {
      font-size: 11px;
      color: var(--muted);
      display: flex;
      justify-content: space-between;
      margin-top: 6px;
    }

    .split-grid {
      display: grid;
      grid-template-columns: 120px 1fr;
      gap: 10px;
      align-items: center;
    }

    .donut {
      width: 104px;
      height: 104px;
      border-radius: 50%;
      background: conic-gradient(var(--json) 0deg var(--json-deg), var(--text) var(--json-deg) 360deg);
      position: relative;
      border: 1px solid var(--line);
      margin: 0 auto;
    }

    .donut::after {
      content: "";
      position: absolute;
      inset: 20px;
      border-radius: 50%;
      background: var(--card-strong);
      border: 1px solid var(--line-soft);
    }

    .donut-center {
      position: absolute;
      inset: 0;
      display: grid;
      place-items: center;
      z-index: 1;
      font-size: 12px;
      color: #d8e6f9;
      font-weight: 650;
    }

    .legend {
      display: grid;
      gap: 8px;
      font-size: 12px;
    }

    .legend-item {
      display: grid;
      grid-template-columns: 12px 1fr auto;
      gap: 8px;
      align-items: center;
    }

    .swatch { width: 10px; height: 10px; border-radius: 2px; }

    .swatch.json { background: var(--json); }
    .swatch.text { background: var(--text); }

    .session-bars {
      display: grid;
      gap: 6px;
      margin-top: 4px;
      font-size: 11px;
    }

    .session-row {
      display: grid;
      grid-template-columns: 1fr 62px;
      gap: 8px;
      align-items: center;
    }

    .session-name {
      color: #d7e6fb;
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
      margin-bottom: 3px;
    }

    .session-track {
      background: #10213a;
      height: 8px;
      border-radius: 999px;
      border: 1px solid var(--line);
      overflow: hidden;
    }

    .session-fill {
      height: 100%;
      background: linear-gradient(90deg, #468fe0, #61b2ff);
      border-radius: 999px;
    }

    .session-val {
      text-align: right;
      color: var(--muted);
      font-variant-numeric: tabular-nums;
    }

    .recent-events {
      display: grid;
      gap: 7px;
      margin-top: 4px;
    }

    .recent-item {
      border: 1px solid var(--line);
      background: #0f1b2f;
      border-radius: 6px;
      padding: 8px;
    }

    .recent-head {
      display: flex;
      justify-content: space-between;
      align-items: baseline;
      gap: 8px;
      font-size: 11px;
      color: var(--muted);
      margin-bottom: 4px;
    }

    .recent-session {
      color: #d7e6fb;
      max-width: 65%;
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
    }

    .recent-type {
      color: #9ec5f2;
      font-size: 10px;
      letter-spacing: 0.4px;
      text-transform: uppercase;
      margin-bottom: 3px;
    }

    .recent-body {
      font-size: 12px;
      line-height: 1.35;
      color: #e9f2fd;
      word-break: break-word;
    }

    table { width: 100%; border-collapse: collapse; }
    th, td {
      text-align: left;
      font-size: 12px;
      padding: 7px 6px;
      border-bottom: 1px solid var(--line);
    }

    th {
      color: var(--muted);
      font-weight: 600;
      font-size: 11px;
      letter-spacing: 0.35px;
      text-transform: uppercase;
    }

    tr:last-child td { border-bottom: none; }
    tbody tr:hover { background: rgba(67, 110, 168, 0.19); }

    .table-card {
      max-height: 350px;
      overflow: auto;
    }

    .foot {
      margin-top: 4px;
      color: var(--muted);
      font-size: 11px;
    }

    @keyframes card-enter {
      from {
        opacity: 0;
        transform: translateY(5px);
      }
      to {
        opacity: 1;
        transform: translateY(0);
      }
    }
  </style>
</head>
<body>
  <div class="wrap">
    <div class="header">
      <div>
        <h1>Claude Logs Dashboard</h1>
        <div class="sub" id="sub">Loading...</div>
      </div>
      <div class="sub" id="header-meta">Auto refresh</div>
    </div>

    <div class="kpi-grid">
      <div class="card kpi"><div class="k">Events</div><div class="v" id="events">-</div></div>
      <div class="card kpi"><div class="k">Sessions</div><div class="v" id="sessions">-</div></div>
      <div class="card kpi"><div class="k">Day Shards</div><div class="v" id="days">-</div></div>
      <div class="card kpi"><div class="k">Files</div><div class="v" id="files">-</div></div>
      <div class="card kpi"><div class="k">Storage</div><div class="v" id="bytes">-</div></div>
      <div class="card kpi"><div class="k">Events / Day</div><div class="v-small" id="events-per-day">-</div></div>
      <div class="card kpi"><div class="k">Events / Session</div><div class="v-small" id="events-per-session">-</div></div>
      <div class="card kpi"><div class="k">Top Session Share</div><div class="v-small" id="top-share">-</div></div>
      <div class="card kpi"><div class="k">JSON Ratio</div><div class="v-small" id="json-ratio">-</div></div>
      <div class="card kpi"><div class="k">7 Day Events</div><div class="v-small" id="events-7d">-</div></div>
    </div>

    <div class="main">
      <div class="panel">
        <div class="card">
          <h2>Most Recent Events</h2>
          <div class="recent-events" id="recent-events"></div>
        </div>

        <div class="card chart-card">
          <h2>Daily Event Trend (latest 21 shards)</h2>
          <div>
            <div class="bars" id="trend-bars"></div>
            <div class="chart-meta">
              <span id="trend-min">Min: -</span>
              <span id="trend-max">Max: -</span>
              <span id="trend-total">Total: -</span>
            </div>
          </div>
        </div>

        <div class="card table-card">
          <h2>Daily Volume</h2>
          <table id="days-table">
            <thead><tr><th>Day</th><th>Events</th><th>Sessions</th><th>Files</th><th>Storage</th></tr></thead>
            <tbody></tbody>
          </table>
        </div>
      </div>

      <div class="panel">
        <div class="card">
          <h2>Type Split</h2>
          <div class="split-grid">
            <div class="donut" id="types-donut" style="--json-deg:180deg;">
              <div class="donut-center" id="donut-center">50%</div>
            </div>
            <div class="legend" id="types-legend"></div>
          </div>
        </div>

        <div class="card">
          <h2>Session Concentration (Top 8)</h2>
          <div class="session-bars" id="session-bars"></div>
        </div>
      </div>
    </div>

    <div class="card table-card">
      <h2>Top Sessions</h2>
      <table id="sessions-table">
        <thead><tr><th>Session</th><th>Events</th><th>Days</th><th>Files</th><th>Last Seen</th></tr></thead>
        <tbody></tbody>
      </table>
    </div>

    <div class="foot" id="foot"></div>
  </div>

  <script>
    const REFRESH_MS = REFRESH_MS_PLACEHOLDER;

    function fmtInt(value) {
      return new Intl.NumberFormat().format(value || 0);
    }

    function fmtBytes(bytes) {
      if (!bytes) return '0 B';
      const units = ['B', 'KB', 'MB', 'GB', 'TB'];
      let i = 0;
      let n = bytes;
      while (n >= 1024 && i < units.length - 1) {
        n /= 1024;
        i += 1;
      }
      return `${n.toFixed(n >= 10 || i === 0 ? 0 : 1)} ${units[i]}`;
    }

    function fmtPct(value) {
      if (!Number.isFinite(value)) return '0.0%';
      return `${value.toFixed(1)}%`;
    }

    function shortTs(value) {
      if (!value) return 'n/a';
      const d = new Date(value);
      if (Number.isNaN(d.getTime())) return value;
      return d.toISOString().replace('T', ' ').slice(0, 16) + 'Z';
    }

    function setText(id, text) {
      document.getElementById(id).textContent = text;
    }

    function renderDays(days) {
      const tbody = document.querySelector('#days-table tbody');
      tbody.innerHTML = '';
      for (const item of days.slice(0, 21)) {
        const tr = document.createElement('tr');
        tr.innerHTML = `<td>${item.day}</td><td>${fmtInt(item.events)}</td><td>${fmtInt(item.sessions)}</td><td>${fmtInt(item.files)}</td><td>${fmtBytes(item.bytes)}</td>`;
        tbody.appendChild(tr);
      }
    }

    function renderTypes(types) {
      const entries = Object.entries(types || {}).map(([name, rec]) => ({ name, events: rec.events || 0 }));
      const total = entries.reduce((sum, item) => sum + item.events, 0);

      const jsonEvents = (types.json && types.json.events) || 0;
      const textEvents = (types.text && types.text.events) || 0;
      const jsonPct = total > 0 ? (jsonEvents / total) * 100 : 0;
      const donut = document.getElementById('types-donut');
      donut.style.setProperty('--json-deg', `${(jsonPct / 100) * 360}deg`);
      setText('donut-center', fmtPct(jsonPct));

      const root = document.getElementById('types-legend');
      root.innerHTML = '';
      for (const item of entries) {
        const pct = total > 0 ? ((item.events / total) * 100) : 0;
        const row = document.createElement('div');
        row.className = 'legend-item';
        row.innerHTML = `
          <div class="swatch ${item.name}"></div>
          <div>${item.name.toUpperCase()} (${fmtPct(pct)})</div>
          <div>${fmtInt(item.events)}</div>
        `;
        root.appendChild(row);
      }
    }

    function renderTrend(days) {
      const items = (days || []).slice(0, 21).reverse();
      const maxEvents = Math.max(...items.map((x) => x.events || 0), 1);
      const minEvents = items.length ? Math.min(...items.map((x) => x.events || 0)) : 0;
      const totalEvents = items.reduce((sum, x) => sum + (x.events || 0), 0);

      const root = document.getElementById('trend-bars');
      root.innerHTML = '';
      for (const item of items) {
        const col = document.createElement('div');
        col.className = 'bar-col';
        const pct = Math.max(2, Math.round(((item.events || 0) / maxEvents) * 100));
        col.style.height = `${pct}%`;
        col.title = `${item.day}: ${fmtInt(item.events || 0)} events`;
        root.appendChild(col);
      }

      setText('trend-min', `Min: ${fmtInt(minEvents)}`);
      setText('trend-max', `Max: ${fmtInt(maxEvents)}`);
      setText('trend-total', `Total: ${fmtInt(totalEvents)}`);
    }

    function renderSessionBars(sessions, totalEvents) {
      const items = (sessions || []).slice(0, 8);
      const maxEvents = Math.max(...items.map((x) => x.events || 0), 1);
      const root = document.getElementById('session-bars');
      root.innerHTML = '';
      for (const item of items) {
        const pct = Math.max(2, Math.round(((item.events || 0) / maxEvents) * 100));
        const share = totalEvents > 0 ? ((item.events || 0) / totalEvents) * 100 : 0;
        const row = document.createElement('div');
        row.className = 'session-row';
        row.innerHTML = `
          <div>
            <div class="session-name">${item.session}</div>
            <div class="session-track"><div class="session-fill" style="width:${pct}%"></div></div>
          </div>
          <div class="session-val">${fmtPct(share)}</div>
        `;
        root.appendChild(row);
      }
    }

    function renderSessions(sessions) {
      const tbody = document.querySelector('#sessions-table tbody');
      tbody.innerHTML = '';
      for (const item of sessions || []) {
        const tr = document.createElement('tr');
        tr.innerHTML = `<td>${item.session}</td><td>${fmtInt(item.events)}</td><td>${fmtInt(item.days)}</td><td>${fmtInt(item.files)}</td><td>${shortTs(item.last_seen)}</td>`;
        tbody.appendChild(tr);
      }
    }

    function renderRecentEvents(events) {
      const root = document.getElementById('recent-events');
      root.innerHTML = '';
      const items = (events || []).slice(0, 5);

      if (!items.length) {
        const empty = document.createElement('div');
        empty.className = 'recent-item';
        empty.textContent = 'No events yet.';
        root.appendChild(empty);
        return;
      }

      for (const item of items) {
        const row = document.createElement('div');
        row.className = 'recent-item';

        const head = document.createElement('div');
        head.className = 'recent-head';

        const session = document.createElement('div');
        session.className = 'recent-session';
        session.textContent = item.session || 'unknown';

        const ts = document.createElement('div');
        ts.textContent = shortTs(item.timestamp);

        head.appendChild(session);
        head.appendChild(ts);

        const eventType = document.createElement('div');
        eventType.className = 'recent-type';
        eventType.textContent = item.event_type || 'event';

        const body = document.createElement('div');
        body.className = 'recent-body';
        body.textContent = item.preview || '(event)';

        row.appendChild(head);
        row.appendChild(eventType);
        row.appendChild(body);
        root.appendChild(row);
      }
    }

    function renderDerived(data) {
      const totals = data.totals || {};
      const days = Math.max(1, totals.days || 0);
      const sessions = Math.max(1, totals.sessions || 0);
      const events = totals.events || 0;
      const top = (data.top_sessions && data.top_sessions[0] && data.top_sessions[0].events) || 0;
      const jsonEvents = (data.types && data.types.json && data.types.json.events) || 0;
      const jsonRatio = events > 0 ? (jsonEvents / events) * 100 : 0;
      const events7d = (data.days || []).slice(0, 7).reduce((sum, x) => sum + (x.events || 0), 0);

      setText('events-per-day', fmtInt(Math.round(events / days)));
      setText('events-per-session', fmtInt(Math.round(events / sessions)));
      setText('top-share', fmtPct(events > 0 ? (top / events) * 100 : 0));
      setText('json-ratio', fmtPct(jsonRatio));
      setText('events-7d', fmtInt(events7d));
    }

    async function refresh() {
      try {
        const response = await fetch('/api/summary', { cache: 'no-store' });
        if (!response.ok) throw new Error(`HTTP ${response.status}`);
        const data = await response.json();

        setText('sub', `Root: ${data.root}`);
        setText('header-meta', `Refresh ${Math.floor(REFRESH_MS / 1000)}s`);
        setText('events', fmtInt(data.totals.events));
        setText('sessions', fmtInt(data.totals.sessions));
        setText('days', fmtInt(data.totals.days));
        setText('files', fmtInt(data.totals.files));
        setText('bytes', fmtBytes(data.totals.bytes));

        renderDays(data.days || []);
        renderTypes(data.types || {});
        renderTrend(data.days || []);
        renderSessionBars(data.top_sessions || [], data.totals.events || 0);
        renderSessions(data.top_sessions || []);
        renderRecentEvents(data.recent_events || []);
        renderDerived(data);

        const newest = data.totals.newest_file_mtime || 'n/a';
        setText('foot', `Updated ${shortTs(data.generated_at)}. Newest file mtime: ${shortTs(newest)}.`);
      } catch (err) {
        setText('sub', `Failed to load data: ${String(err)}`);
      }
    }

    refresh();
    setInterval(refresh, REFRESH_MS);
  </script>
</body>
</html>
"#;

    template.replace("REFRESH_MS_PLACEHOLDER", &refresh_ms.to_string())
}
