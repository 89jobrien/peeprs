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
    let stats = scan_event_lines(reader, &ef).unwrap();
    assert_eq!(stats.event_count, 0);
    assert!(stats.session_stats.is_empty());
    assert!(stats.recent_events.is_empty());
}

#[test]
fn test_scan_event_lines_valid_json() {
    let data = r#"{"session":"s1","type":"tool_use","display":"hi","timestamp":"2023-11-14T22:13:20.000Z"}
{"session":"s1","type":"tool_use","display":"bye","timestamp":"2023-11-14T22:14:20.000Z"}
"#;
    let reader = Cursor::new(data.as_bytes());
    let ef = make_event_file("fallback", "api");
    let stats = scan_event_lines(reader, &ef).unwrap();
    assert_eq!(stats.event_count, 2);
    assert_eq!(stats.session_stats["s1"].events, 2);
    assert_eq!(stats.recent_events.len(), 2);
}

#[test]
fn test_scan_event_lines_invalid_json_uses_fallback() {
    let data = "not json\n";
    let reader = Cursor::new(data.as_bytes());
    let ef = make_event_file("fallback-sess", "api");
    let stats = scan_event_lines(reader, &ef).unwrap();
    assert_eq!(stats.event_count, 1);
    assert_eq!(stats.session_stats["fallback-sess"].events, 1);
}

#[test]
fn test_scan_event_lines_mixed() {
    let data = "not json\n{\"session\":\"s1\",\"display\":\"ok\"}\n";
    let reader = Cursor::new(data.as_bytes());
    let ef = make_event_file("fallback", "api");
    let stats = scan_event_lines(reader, &ef).unwrap();
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

#[test]
fn test_iter_event_files_flat_layout() {
    let dir = tempfile::tempdir().unwrap();
    let session_dir = dir.path().join("2025-01-15").join("sess-abc");
    fs::create_dir_all(&session_dir).unwrap();
    fs::write(session_dir.join("events.jsonl"), "{}").unwrap();

    let result = iter_event_files(dir.path()).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].day, "2025-01-15");
    assert_eq!(result[0].session, "sess-abc");
    assert_eq!(result[0].channel, "");
    assert_eq!(result[0].file_type, "json");
}

#[test]
fn test_iter_event_files_both_layouts() {
    let dir = tempfile::tempdir().unwrap();

    // Flat: day/session/events.jsonl
    let flat_dir = dir.path().join("2025-01-15").join("sess-flat");
    fs::create_dir_all(&flat_dir).unwrap();
    fs::write(flat_dir.join("events.jsonl"), "{}").unwrap();

    // Nested: day/session/channel/events.jsonl
    let nested_dir = dir.path().join("2025-01-15").join("sess-nested").join("api");
    fs::create_dir_all(&nested_dir).unwrap();
    fs::write(nested_dir.join("events.jsonl"), "{}").unwrap();

    let result = iter_event_files(dir.path()).unwrap();
    assert_eq!(result.len(), 2);
}

// --- build_summary ---

#[tokio::test]
async fn test_build_summary_empty_dir() {
    let dir = tempfile::tempdir().unwrap();
    let summary = build_summary(dir.path(), None).await.unwrap();
    assert_eq!(summary.totals.files, 0);
    assert_eq!(summary.totals.events, 0);
    assert!(summary.days.is_empty());
    assert!(summary.top_sessions.is_empty());
}

#[tokio::test]
async fn test_build_summary_with_events() {
    let dir = tempfile::tempdir().unwrap();
    let channel_dir = dir.path().join("2025-01-15").join("sess-1").join("api");
    fs::create_dir_all(&channel_dir).unwrap();
    let line = r#"{"session":"sess-1","type":"tool_use","display":"hi","timestamp":"2023-11-14T22:13:20.000Z"}"#;
    fs::write(channel_dir.join("events.jsonl"), format!("{line}\n")).unwrap();

    let summary = build_summary(dir.path(), None).await.unwrap();
    assert_eq!(summary.totals.files, 1);
    assert_eq!(summary.totals.events, 1);
    assert_eq!(summary.totals.days, 1);
    assert_eq!(summary.totals.sessions, 1);
    assert_eq!(summary.days.len(), 1);
    assert_eq!(summary.days[0].day, "2025-01-15");
}

#[tokio::test]
async fn test_build_summary_with_agent_field() {
    let dir = tempfile::tempdir().unwrap();
    let channel_dir = dir.path().join("2025-01-15").join("sess-1").join("api");
    fs::create_dir_all(&channel_dir).unwrap();
    let line = r#"{"session":"sess-1","type":"tool_use","display":"hi","timestamp":"2023-11-14T22:13:20.000Z","agent":"gemini"}"#;
    fs::write(channel_dir.join("events.jsonl"), format!("{line}\n")).unwrap();

    let summary = build_summary(dir.path(), None).await.unwrap();
    assert_eq!(summary.agents.len(), 1);
    assert!(summary.agents.contains_key("gemini"));
    let gemini = &summary.agents["gemini"];
    assert_eq!(gemini.events, 1);
    assert_eq!(gemini.sessions, 1);
    assert_eq!(gemini.files, 1);
    assert_eq!(summary.top_sessions[0].agent, "gemini");
}

#[tokio::test]
async fn test_build_summary_without_agent_defaults_unknown() {
    let dir = tempfile::tempdir().unwrap();
    let channel_dir = dir.path().join("2025-01-15").join("sess-1").join("api");
    fs::create_dir_all(&channel_dir).unwrap();
    let line = r#"{"session":"sess-1","type":"tool_use","display":"hi","timestamp":"2023-11-14T22:13:20.000Z"}"#;
    fs::write(channel_dir.join("events.jsonl"), format!("{line}\n")).unwrap();

    let summary = build_summary(dir.path(), None).await.unwrap();
    assert_eq!(summary.agents.len(), 1);
    assert!(summary.agents.contains_key("unknown"));
    assert_eq!(summary.top_sessions[0].agent, "unknown");
}

// --- new field accumulation ---

#[test]
fn test_scan_event_lines_token_accumulation() {
    let data = r#"{"session":"s1","type":"assistant","timestamp":"2023-11-14T22:13:20.000Z","message":{"usage":{"input_tokens":100,"output_tokens":50,"cache_read_input_tokens":200,"cache_creation_input_tokens":30}}}
{"session":"s1","type":"assistant","timestamp":"2023-11-14T22:14:20.000Z","message":{"usage":{"input_tokens":80,"output_tokens":40,"cache_read_input_tokens":0,"cache_creation_input_tokens":10}}}
"#;
    let reader = Cursor::new(data.as_bytes());
    let ef = make_event_file("fallback", "api");
    let stats = scan_event_lines(reader, &ef).unwrap();
    assert_eq!(stats.token_usage.input_tokens, 180);
    assert_eq!(stats.token_usage.output_tokens, 90);
    assert_eq!(stats.token_usage.cache_read_input_tokens, 200);
    assert_eq!(stats.token_usage.cache_creation_input_tokens, 40);
}

#[test]
fn test_scan_event_lines_tool_counts() {
    let data = r#"{"session":"s1","type":"assistant","timestamp":"2023-11-14T22:13:20.000Z","message":{"content":[{"type":"tool_use","name":"Read","input":{}},{"type":"tool_use","name":"Write","input":{}}]}}
{"session":"s1","type":"assistant","timestamp":"2023-11-14T22:14:20.000Z","message":{"content":[{"type":"tool_use","name":"Read","input":{}}]}}
"#;
    let reader = Cursor::new(data.as_bytes());
    let ef = make_event_file("fallback", "api");
    let stats = scan_event_lines(reader, &ef).unwrap();
    assert_eq!(stats.tool_counts.get("Read"), Some(&2));
    assert_eq!(stats.tool_counts.get("Write"), Some(&1));
}

#[test]
fn test_scan_event_lines_turn_durations() {
    let data = r#"{"session":"s1","type":"system","subtype":"turn_duration","durationMs":5000,"timestamp":"2023-11-14T22:13:20.000Z"}
{"session":"s1","type":"system","subtype":"turn_duration","durationMs":12000,"timestamp":"2023-11-14T22:14:20.000Z"}
"#;
    let reader = Cursor::new(data.as_bytes());
    let ef = make_event_file("fallback", "api");
    let stats = scan_event_lines(reader, &ef).unwrap();
    assert_eq!(stats.turn_durations, vec![5000, 12000]);
}

#[test]
fn test_scan_event_lines_error_counts() {
    let data = r#"{"session":"s1","type":"assistant","timestamp":"2023-11-14T22:13:20.000Z","isApiErrorMessage":true}
{"session":"s1","type":"assistant","timestamp":"2023-11-14T22:14:20.000Z","message":{"content":[{"type":"tool_result","is_error":true,"content":"fail"}]}}
{"session":"s1","type":"assistant","timestamp":"2023-11-14T22:15:20.000Z","display":"ok"}
"#;
    let reader = Cursor::new(data.as_bytes());
    let ef = make_event_file("fallback", "api");
    let stats = scan_event_lines(reader, &ef).unwrap();
    assert_eq!(stats.api_error_count, 1);
    assert_eq!(stats.tool_error_count, 1);
}

#[tokio::test]
async fn test_build_summary_token_usage() {
    let dir = tempfile::tempdir().unwrap();
    let channel_dir = dir.path().join("2025-01-15").join("sess-1").join("api");
    fs::create_dir_all(&channel_dir).unwrap();
    let line = r#"{"session":"sess-1","type":"assistant","timestamp":"2023-11-14T22:13:20.000Z","message":{"usage":{"input_tokens":100,"output_tokens":50,"cache_read_input_tokens":200,"cache_creation_input_tokens":30}}}"#;
    fs::write(channel_dir.join("events.jsonl"), format!("{line}\n")).unwrap();

    let summary = build_summary(dir.path(), None).await.unwrap();
    assert_eq!(summary.token_usage.total_input, 100);
    assert_eq!(summary.token_usage.total_output, 50);
    assert_eq!(summary.token_usage.total_cache_read, 200);
    assert_eq!(summary.token_usage.total_cache_creation, 30);
    assert_eq!(summary.token_usage.daily.len(), 1);
    assert_eq!(summary.token_usage.daily[0].input_tokens, 100);
}

#[tokio::test]
async fn test_build_summary_error_rate() {
    let dir = tempfile::tempdir().unwrap();
    let channel_dir = dir.path().join("2025-01-15").join("sess-1").join("api");
    fs::create_dir_all(&channel_dir).unwrap();
    let lines = r#"{"session":"sess-1","type":"assistant","timestamp":"2023-11-14T22:13:20.000Z","isApiErrorMessage":true}
{"session":"sess-1","type":"assistant","timestamp":"2023-11-14T22:14:20.000Z","display":"ok"}
"#;
    fs::write(channel_dir.join("events.jsonl"), lines).unwrap();

    let summary = build_summary(dir.path(), None).await.unwrap();
    assert_eq!(summary.error_rate.api_errors, 1);
    assert_eq!(summary.error_rate.total_events, 2);
    assert!((summary.error_rate.api_error_rate - 0.5).abs() < 0.01);
}

// --- cache integration ---

#[tokio::test]
async fn test_build_summary_with_cache() {
    use peeprs::cache::ScanCache;

    let dir = tempfile::tempdir().unwrap();
    let channel_dir = dir.path().join("2025-01-15").join("sess-1").join("api");
    fs::create_dir_all(&channel_dir).unwrap();
    let line = r#"{"session":"sess-1","type":"tool_use","display":"hi","timestamp":"2023-11-14T22:13:20.000Z"}"#;
    fs::write(channel_dir.join("events.jsonl"), format!("{line}\n")).unwrap();

    let cache_dir = tempfile::tempdir().unwrap();
    let cache_db = cache_dir.path().join("test.db");
    let cache = ScanCache::open(&cache_db).await.unwrap();

    // First call populates cache
    let summary1 = build_summary(dir.path(), Some(&cache)).await.unwrap();
    assert_eq!(summary1.totals.files, 1);
    assert_eq!(summary1.totals.events, 1);

    // Second call should hit cache (same result)
    let summary2 = build_summary(dir.path(), Some(&cache)).await.unwrap();
    assert_eq!(summary2.totals.files, 1);
    assert_eq!(summary2.totals.events, 1);
    assert_eq!(summary2.totals.sessions, summary1.totals.sessions);
}

#[tokio::test]
async fn test_cache_miss_on_modified_file() {
    use peeprs::cache::ScanCache;

    let dir = tempfile::tempdir().unwrap();
    let channel_dir = dir.path().join("2025-01-15").join("sess-1").join("api");
    fs::create_dir_all(&channel_dir).unwrap();
    let line = r#"{"session":"sess-1","type":"tool_use","display":"hi","timestamp":"2023-11-14T22:13:20.000Z"}"#;
    let event_path = channel_dir.join("events.jsonl");
    fs::write(&event_path, format!("{line}\n")).unwrap();

    let cache_dir = tempfile::tempdir().unwrap();
    let cache_db = cache_dir.path().join("test.db");
    let cache = ScanCache::open(&cache_db).await.unwrap();

    // Populate cache
    let summary1 = build_summary(dir.path(), Some(&cache)).await.unwrap();
    assert_eq!(summary1.totals.events, 1);

    // Modify the file (add a second event)
    let line2 = r#"{"session":"sess-1","type":"tool_use","display":"bye","timestamp":"2023-11-14T22:14:20.000Z"}"#;
    fs::write(&event_path, format!("{line}\n{line2}\n")).unwrap();

    // Should re-scan because size changed
    let summary2 = build_summary(dir.path(), Some(&cache)).await.unwrap();
    assert_eq!(summary2.totals.events, 2);
}

#[tokio::test]
async fn test_cache_stale_cleanup() {
    use peeprs::cache::ScanCache;

    let dir = tempfile::tempdir().unwrap();
    let channel_dir = dir.path().join("2025-01-15").join("sess-1").join("api");
    fs::create_dir_all(&channel_dir).unwrap();
    let line = r#"{"session":"sess-1","type":"tool_use","display":"hi","timestamp":"2023-11-14T22:13:20.000Z"}"#;
    let event_path = channel_dir.join("events.jsonl");
    fs::write(&event_path, format!("{line}\n")).unwrap();

    let cache_dir = tempfile::tempdir().unwrap();
    let cache_db = cache_dir.path().join("test.db");
    let cache = ScanCache::open(&cache_db).await.unwrap();

    // Populate cache
    build_summary(dir.path(), Some(&cache)).await.unwrap();

    // Remove the file
    fs::remove_file(&event_path).unwrap();

    // Build again — stale entry should be cleaned up
    let summary = build_summary(dir.path(), Some(&cache)).await.unwrap();
    assert_eq!(summary.totals.files, 0);
}
