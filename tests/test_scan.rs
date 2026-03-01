use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use peeprs::models::{EventFile, RecentEvent, RecentEventAccum};
use peeprs::scan::*;

fn make_event_file(session: &str, channel: &str) -> EventFile {
    EventFile {
        day: "2025-01-01".to_string(),
        session: session.to_string(),
        channel: channel.to_string(),
        file_type: "json".to_string(),
        path: PathBuf::from("/tmp/fake"),
    }
}

// --- scan_event_lines ---

#[test]
fn test_scan_event_lines_empty() {
    let reader = Cursor::new(b"" as &[u8]);
    let ef = make_event_file("sess", "api");
    let mut pool = Vec::new();
    let stats = scan_event_lines(reader, &ef, &mut pool).unwrap();
    assert_eq!(stats.event_count, 0);
    assert!(stats.session_stats.is_empty());
}

#[test]
fn test_scan_event_lines_valid_json() {
    let data = r#"{"session":"s1","type":"tool_use","display":"hi","timestamp":"2023-11-14T22:13:20.000Z"}
{"session":"s1","type":"tool_use","display":"bye","timestamp":"2023-11-14T22:14:20.000Z"}
"#;
    let reader = Cursor::new(data.as_bytes());
    let ef = make_event_file("fallback", "api");
    let mut pool = Vec::new();
    let stats = scan_event_lines(reader, &ef, &mut pool).unwrap();
    assert_eq!(stats.event_count, 2);
    assert_eq!(stats.session_stats["s1"].events, 2);
    assert_eq!(pool.len(), 2);
}

#[test]
fn test_scan_event_lines_invalid_json_uses_fallback() {
    let data = "not json\n";
    let reader = Cursor::new(data.as_bytes());
    let ef = make_event_file("fallback-sess", "api");
    let mut pool = Vec::new();
    let stats = scan_event_lines(reader, &ef, &mut pool).unwrap();
    assert_eq!(stats.event_count, 1);
    assert_eq!(stats.session_stats["fallback-sess"].events, 1);
}

#[test]
fn test_scan_event_lines_mixed() {
    let data = "not json\n{\"session\":\"s1\",\"display\":\"ok\"}\n";
    let reader = Cursor::new(data.as_bytes());
    let ef = make_event_file("fallback", "api");
    let mut pool = Vec::new();
    let stats = scan_event_lines(reader, &ef, &mut pool).unwrap();
    assert_eq!(stats.event_count, 2);
    assert_eq!(stats.session_stats["fallback"].events, 1);
    assert_eq!(stats.session_stats["s1"].events, 1);
}

// --- insert_recent_event ---

#[test]
fn test_insert_recent_event_keeps_top_5() {
    let mut pool = Vec::new();
    for i in 0..10 {
        let event = RecentEventAccum {
            sort_key_ms: i,
            event: RecentEvent {
                timestamp: format!("ts-{i}"),
                session: "s".to_string(),
                agent: "unknown".to_string(),
                event_type: "t".to_string(),
                preview: "p".to_string(),
            },
        };
        insert_recent_event(&mut pool, event);
    }
    assert_eq!(pool.len(), 5);
    assert_eq!(pool[0].sort_key_ms, 9);
    assert_eq!(pool[4].sort_key_ms, 5);
}

#[test]
fn test_insert_recent_event_sorted_descending() {
    let mut pool = Vec::new();
    for ms in [3, 1, 4, 1, 5] {
        let event = RecentEventAccum {
            sort_key_ms: ms,
            event: RecentEvent {
                timestamp: String::new(),
                session: String::new(),
                agent: "unknown".to_string(),
                event_type: String::new(),
                preview: String::new(),
            },
        };
        insert_recent_event(&mut pool, event);
    }
    let keys: Vec<i64> = pool.iter().map(|e| e.sort_key_ms).collect();
    assert_eq!(keys, vec![5, 4, 3, 1, 1]);
}

// --- iter_event_files ---

#[test]
fn test_iter_event_files_nonexistent_dir() {
    let result = iter_event_files(Path::new("/nonexistent/path/xyz")).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_iter_event_files_empty_dir() {
    let dir = tempfile::tempdir().unwrap();
    let result = iter_event_files(dir.path()).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_iter_event_files_skips_bad_day_names() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("not-a-day")).unwrap();
    fs::create_dir(dir.path().join("readme.txt")).unwrap();
    let result = iter_event_files(dir.path()).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_iter_event_files_finds_jsonl() {
    let dir = tempfile::tempdir().unwrap();
    let channel_dir = dir
        .path()
        .join("2025-01-15")
        .join("sess-abc")
        .join("api");
    fs::create_dir_all(&channel_dir).unwrap();
    fs::write(channel_dir.join("events.jsonl"), "{}").unwrap();

    let result = iter_event_files(dir.path()).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].day, "2025-01-15");
    assert_eq!(result[0].session, "sess-abc");
    assert_eq!(result[0].channel, "api");
    assert_eq!(result[0].file_type, "json");
}

#[test]
fn test_iter_event_files_text_channel() {
    let dir = tempfile::tempdir().unwrap();
    let channel_dir = dir
        .path()
        .join("2025-01-15")
        .join("sess-abc")
        .join("text");
    fs::create_dir_all(&channel_dir).unwrap();
    fs::write(channel_dir.join("events.jsonl"), "{}").unwrap();

    let result = iter_event_files(dir.path()).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].file_type, "text");
}

// --- build_summary ---

#[test]
fn test_build_summary_empty_dir() {
    let dir = tempfile::tempdir().unwrap();
    let summary = build_summary(dir.path()).unwrap();
    assert_eq!(summary.totals.files, 0);
    assert_eq!(summary.totals.events, 0);
    assert!(summary.days.is_empty());
    assert!(summary.top_sessions.is_empty());
}

#[test]
fn test_build_summary_with_events() {
    let dir = tempfile::tempdir().unwrap();
    let channel_dir = dir.path().join("2025-01-15").join("sess-1").join("api");
    fs::create_dir_all(&channel_dir).unwrap();
    let line = r#"{"session":"sess-1","type":"tool_use","display":"hi","timestamp":"2023-11-14T22:13:20.000Z"}"#;
    fs::write(channel_dir.join("events.jsonl"), format!("{line}\n")).unwrap();

    let summary = build_summary(dir.path()).unwrap();
    assert_eq!(summary.totals.files, 1);
    assert_eq!(summary.totals.events, 1);
    assert_eq!(summary.totals.days, 1);
    assert_eq!(summary.totals.sessions, 1);
    assert_eq!(summary.days.len(), 1);
    assert_eq!(summary.days[0].day, "2025-01-15");
}

#[test]
fn test_build_summary_with_agent_field() {
    let dir = tempfile::tempdir().unwrap();
    let channel_dir = dir.path().join("2025-01-15").join("sess-1").join("api");
    fs::create_dir_all(&channel_dir).unwrap();
    let line = r#"{"session":"sess-1","type":"tool_use","display":"hi","timestamp":"2023-11-14T22:13:20.000Z","agent":"gemini"}"#;
    fs::write(channel_dir.join("events.jsonl"), format!("{line}\n")).unwrap();

    let summary = build_summary(dir.path()).unwrap();
    assert_eq!(summary.agents.len(), 1);
    assert!(summary.agents.contains_key("gemini"));
    let gemini = &summary.agents["gemini"];
    assert_eq!(gemini.events, 1);
    assert_eq!(gemini.sessions, 1);
    assert_eq!(gemini.files, 1);
    assert_eq!(summary.top_sessions[0].agent, "gemini");
}

#[test]
fn test_build_summary_without_agent_defaults_unknown() {
    let dir = tempfile::tempdir().unwrap();
    let channel_dir = dir.path().join("2025-01-15").join("sess-1").join("api");
    fs::create_dir_all(&channel_dir).unwrap();
    let line = r#"{"session":"sess-1","type":"tool_use","display":"hi","timestamp":"2023-11-14T22:13:20.000Z"}"#;
    fs::write(channel_dir.join("events.jsonl"), format!("{line}\n")).unwrap();

    let summary = build_summary(dir.path()).unwrap();
    assert_eq!(summary.agents.len(), 1);
    assert!(summary.agents.contains_key("unknown"));
    assert_eq!(summary.top_sessions[0].agent, "unknown");
}
