use std::collections::{HashMap, HashSet};
use std::ops::AddAssign;
use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone, Serialize, Default)]
pub struct AgentStats {
    pub events: u64,
    pub bytes: u64,
    pub sessions: u64,
    pub files: u64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct TokenUsage {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_input_tokens: i64,
    pub cache_creation_input_tokens: i64,
}

impl AddAssign for TokenUsage {
    fn add_assign(&mut self, rhs: Self) {
        self.input_tokens += rhs.input_tokens;
        self.output_tokens += rhs.output_tokens;
        self.cache_read_input_tokens += rhs.cache_read_input_tokens;
        self.cache_creation_input_tokens += rhs.cache_creation_input_tokens;
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct HookInfo {
    pub command: String,
    pub duration_ms: i64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct HookAccum {
    pub count: u64,
    pub total_ms: i64,
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
    pub token_usage: TokenUsageSummary,
    pub tool_usage: Vec<ToolUsageRow>,
    pub turn_durations: TurnDurationSummary,
    pub session_timeline: Vec<SessionTimelineRow>,
    pub cache_efficiency: CacheEfficiency,
    pub error_rate: ErrorRate,
    pub hook_performance: Vec<HookPerfRow>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct TokenUsageSummary {
    pub total_input: i64,
    pub total_output: i64,
    pub total_cache_read: i64,
    pub total_cache_creation: i64,
    pub daily: Vec<DayTokenRow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DayTokenRow {
    pub day: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_creation_tokens: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolUsageRow {
    pub tool: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct TurnDurationSummary {
    pub buckets: Vec<DurationBucket>,
    pub min_ms: i64,
    pub max_ms: i64,
    pub avg_ms: i64,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DurationBucket {
    pub label: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionTimelineRow {
    pub session: String,
    pub hours: Vec<u64>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct CacheEfficiency {
    pub total_cache_read: i64,
    pub total_cache_creation: i64,
    pub ratio: f64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ErrorRate {
    pub api_errors: u64,
    pub tool_errors: u64,
    pub total_events: u64,
    pub api_error_rate: f64,
    pub tool_error_rate: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct HookPerfRow {
    pub command: String,
    pub count: u64,
    pub avg_ms: i64,
    pub total_ms: i64,
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
    pub recent_events: Vec<RecentEventAccum>,
    pub token_usage: TokenUsage,
    pub tool_counts: HashMap<String, u64>,
    pub turn_durations: Vec<i64>,
    pub hook_stats: HashMap<String, HookAccum>,
    pub api_error_count: u64,
    pub tool_error_count: u64,
    pub model_counts: HashMap<String, u64>,
}

#[derive(Debug, Default)]
pub struct SessionLineStats {
    pub events: u64,
    pub bytes: u64,
    pub last_seen_ms: Option<i64>,
    pub agent: Option<String>,
    pub hourly_events: [u64; 24],
}

#[derive(Debug, Clone)]
pub struct ParsedEventLine {
    pub session: String,
    pub agent: String,
    pub event_type: String,
    pub preview: String,
    pub timestamp_ms: Option<i64>,
    pub timestamp_display: Option<String>,
    pub token_usage: Option<TokenUsage>,
    pub model: Option<String>,
    pub tool_uses: Vec<String>,
    pub turn_duration_ms: Option<i64>,
    pub hook_infos: Vec<HookInfo>,
    pub is_api_error: bool,
    pub is_tool_error: bool,
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
    pub tokens: TokenUsage,
}

#[derive(Debug)]
pub struct EventFile {
    pub day: String,
    pub session: String,
    pub channel: String,
    pub file_type: String,
    pub path: PathBuf,
}
