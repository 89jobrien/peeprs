use std::collections::HashMap;

use peeprs::models::{DashboardSummary, ParsedEventLine, Totals, TypeBuckets};

#[test]
fn test_dashboard_summary_serializes() {
    let summary = DashboardSummary {
        generated_at: "2026-01-01T00:00:00.000Z".to_string(),
        root: "/tmp/logs".to_string(),
        totals: Totals::default(),
        types: TypeBuckets::default(),
        days: vec![],
        top_sessions: vec![],
        recent_events: vec![],
        agents: HashMap::new(),
    };
    let json = serde_json::to_string(&summary).unwrap();
    let round_trip: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(round_trip["root"], "/tmp/logs");
    assert_eq!(round_trip["generated_at"], "2026-01-01T00:00:00.000Z");
}

#[test]
fn test_totals_default() {
    let totals = Totals::default();
    assert_eq!(totals.days, 0);
    assert_eq!(totals.sessions, 0);
    assert_eq!(totals.files, 0);
    assert_eq!(totals.events, 0);
    assert_eq!(totals.bytes, 0);
    assert!(totals.newest_file_mtime.is_none());
}

#[test]
fn test_parsed_event_line_to_recent_event_some() {
    let parsed = ParsedEventLine {
        session: "sess-1".to_string(),
        agent: "claude".to_string(),
        event_type: "tool_use".to_string(),
        preview: "hello".to_string(),
        timestamp_ms: Some(1700000000000),
        timestamp_display: Some("2023-11-14T22:13:20.000Z".to_string()),
    };
    let result = parsed.to_recent_event();
    assert!(result.is_some());
    let accum = result.unwrap();
    assert_eq!(accum.sort_key_ms, 1700000000000);
    assert_eq!(accum.event.session, "sess-1");
    assert_eq!(accum.event.agent, "claude");
    assert_eq!(accum.event.event_type, "tool_use");
    assert_eq!(accum.event.timestamp, "2023-11-14T22:13:20.000Z");
}

#[test]
fn test_parsed_event_line_to_recent_event_none() {
    let parsed = ParsedEventLine {
        session: "sess-1".to_string(),
        agent: "unknown".to_string(),
        event_type: "tool_use".to_string(),
        preview: "hello".to_string(),
        timestamp_ms: None,
        timestamp_display: None,
    };
    assert!(parsed.to_recent_event().is_none());
}
