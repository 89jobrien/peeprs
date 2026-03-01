use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone, Serialize, Default)]
pub struct AgentStats {
    pub events: u64,
    pub bytes: u64,
    pub sessions: u64,
    pub files: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DashboardSummary {
    pub generated_at: String,
    pub root: String,
    pub totals: Totals,
    pub types: TypeBuckets,
    pub days: Vec<DayRow>,
    pub top_sessions: Vec<SessionRow>,
    pub recent_events: Vec<RecentEvent>,
    pub agents: HashMap<String, AgentStats>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Totals {
    pub days: u64,
    pub sessions: u64,
    pub files: u64,
    pub events: u64,
    pub bytes: u64,
    pub newest_file_mtime: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct TypeBuckets {
    pub json: TypeStats,
    pub text: TypeStats,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct TypeStats {
    pub events: u64,
    pub bytes: u64,
    pub files: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DayRow {
    pub day: String,
    pub events: u64,
    pub bytes: u64,
    pub files: u64,
    pub sessions: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionRow {
    pub session: String,
    pub agent: String,
    pub events: u64,
    pub bytes: u64,
    pub files: u64,
    pub days: u64,
    pub last_seen: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecentEvent {
    pub timestamp: String,
    pub session: String,
    pub agent: String,
    pub event_type: String,
    pub preview: String,
}

#[derive(Debug, Clone)]
pub struct RecentEventAccum {
    pub sort_key_ms: i64,
    pub event: RecentEvent,
}

#[derive(Debug, Default)]
pub struct FileScanStats {
    pub event_count: u64,
    pub session_stats: HashMap<String, SessionLineStats>,
}

#[derive(Debug, Default)]
pub struct SessionLineStats {
    pub events: u64,
    pub bytes: u64,
    pub last_seen_ms: Option<i64>,
    pub agent: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ParsedEventLine {
    pub session: String,
    pub agent: String,
    pub event_type: String,
    pub preview: String,
    pub timestamp_ms: Option<i64>,
    pub timestamp_display: Option<String>,
}

impl ParsedEventLine {
    pub fn to_recent_event(&self) -> Option<RecentEventAccum> {
        Some(RecentEventAccum {
            sort_key_ms: self.timestamp_ms?,
            event: RecentEvent {
                timestamp: self.timestamp_display.clone()?,
                session: self.session.clone(),
                agent: self.agent.clone(),
                event_type: self.event_type.clone(),
                preview: self.preview.clone(),
            },
        })
    }
}

#[derive(Debug, Default)]
pub struct SessionAccum {
    pub events: u64,
    pub bytes: u64,
    pub files: u64,
    pub days: HashSet<String>,
    pub last_seen_ms: Option<i64>,
    pub agent: Option<String>,
}

#[derive(Debug, Default)]
pub struct DayAccum {
    pub events: u64,
    pub bytes: u64,
    pub files: u64,
    pub sessions: HashSet<String>,
}

#[derive(Debug)]
pub struct EventFile {
    pub day: String,
    pub session: String,
    pub channel: String,
    pub file_type: String,
    pub path: PathBuf,
}
