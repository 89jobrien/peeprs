use std::path::PathBuf;
use std::time::SystemTime;

use peeprs::models::EventFile;
use peeprs::parse::*;

// --- extract_token_usage ---

#[test]
fn test_extract_token_usage_present() {
    let val = serde_json::json!({
        "message": {
            "usage": {
                "input_tokens": 100,
                "output_tokens": 50,
                "cache_read_input_tokens": 200,
                "cache_creation_input_tokens": 30
            }
        }
    });
    let tu = extract_token_usage(&val).unwrap();
    assert_eq!(tu.input_tokens, 100);
    assert_eq!(tu.output_tokens, 50);
    assert_eq!(tu.cache_read_input_tokens, 200);
    assert_eq!(tu.cache_creation_input_tokens, 30);
}

#[test]
fn test_extract_token_usage_missing() {
    let val = serde_json::json!({"message": {"content": "hi"}});
    assert!(extract_token_usage(&val).is_none());
}

#[test]
fn test_extract_token_usage_partial() {
    let val = serde_json::json!({
        "message": {
            "usage": {
                "input_tokens": 100,
                "output_tokens": 50
            }
        }
    });
    let tu = extract_token_usage(&val).unwrap();
    assert_eq!(tu.input_tokens, 100);
    assert_eq!(tu.cache_read_input_tokens, 0);
}

// --- extract_model ---

#[test]
fn test_extract_model_present() {
    let val = serde_json::json!({"message": {"model": "claude-3-opus-20240229"}});
    assert_eq!(extract_model(&val).unwrap(), "claude-3-opus-20240229");
}

#[test]
fn test_extract_model_missing() {
    let val = serde_json::json!({"message": {"content": "hi"}});
    assert!(extract_model(&val).is_none());
}

#[test]
fn test_extract_model_empty() {
    let val = serde_json::json!({"message": {"model": ""}});
    assert!(extract_model(&val).is_none());
}

// --- extract_tool_uses ---

#[test]
fn test_extract_tool_uses_present() {
    let val = serde_json::json!({
        "message": {
            "content": [
                {"type": "text", "text": "hello"},
                {"type": "tool_use", "name": "Read", "input": {}},
                {"type": "tool_use", "name": "Write", "input": {}}
            ]
        }
    });
    let tools = extract_tool_uses(&val);
    assert_eq!(tools, vec!["Read", "Write"]);
}

#[test]
fn test_extract_tool_uses_none() {
    let val = serde_json::json!({"message": {"content": "text"}});
    assert!(extract_tool_uses(&val).is_empty());
}

// --- extract_turn_duration ---

#[test]
fn test_extract_turn_duration_present() {
    let val = serde_json::json!({"subtype": "turn_duration", "durationMs": 5000});
    assert_eq!(extract_turn_duration(&val), Some(5000));
}

#[test]
fn test_extract_turn_duration_wrong_subtype() {
    let val = serde_json::json!({"subtype": "other", "durationMs": 5000});
    assert!(extract_turn_duration(&val).is_none());
}

#[test]
fn test_extract_turn_duration_missing_subtype() {
    let val = serde_json::json!({"durationMs": 5000});
    assert!(extract_turn_duration(&val).is_none());
}

// --- extract_hook_infos ---

#[test]
fn test_extract_hook_infos_present() {
    let val = serde_json::json!({
        "subtype": "stop_hook_summary",
        "hookInfos": [
            {"command": "lint-hook", "durationMs": 120},
            {"command": "audit-hook", "durationMs": 45}
        ]
    });
    let infos = extract_hook_infos(&val);
    assert_eq!(infos.len(), 2);
    assert_eq!(infos[0].command, "lint-hook");
    assert_eq!(infos[0].duration_ms, 120);
    assert_eq!(infos[1].command, "audit-hook");
}

#[test]
fn test_extract_hook_infos_wrong_subtype() {
    let val = serde_json::json!({"subtype": "other", "hookInfos": []});
    assert!(extract_hook_infos(&val).is_empty());
}

// --- extract_is_api_error ---

#[test]
fn test_extract_is_api_error_true() {
    let val = serde_json::json!({"isApiErrorMessage": true});
    assert!(extract_is_api_error(&val));
}

#[test]
fn test_extract_is_api_error_false() {
    let val = serde_json::json!({"isApiErrorMessage": false});
    assert!(!extract_is_api_error(&val));
}

#[test]
fn test_extract_is_api_error_missing() {
    let val = serde_json::json!({});
    assert!(!extract_is_api_error(&val));
}

// --- extract_is_tool_error ---

#[test]
fn test_extract_is_tool_error_present() {
    let val = serde_json::json!({
        "message": {
            "content": [
                {"type": "tool_result", "is_error": true, "content": "failed"}
            ]
        }
    });
    assert!(extract_is_tool_error(&val));
}

#[test]
fn test_extract_is_tool_error_no_error() {
    let val = serde_json::json!({
        "message": {
            "content": [
                {"type": "tool_result", "is_error": false, "content": "ok"}
            ]
        }
    });
    assert!(!extract_is_tool_error(&val));
}

#[test]
fn test_extract_is_tool_error_missing() {
    let val = serde_json::json!({"message": {"content": "text"}});
    assert!(!extract_is_tool_error(&val));
}

// --- looks_like_day ---

#[test]
fn test_looks_like_day_valid() {
    assert!(looks_like_day("2025-01-15"));
    assert!(looks_like_day("2000-12-31"));
    assert!(looks_like_day("9999-99-99"));
}

#[test]
fn test_looks_like_day_too_short() {
    assert!(!looks_like_day("2025-1-15"));
}

#[test]
fn test_looks_like_day_too_long() {
    assert!(!looks_like_day("2025-01-150"));
}

#[test]
fn test_looks_like_day_wrong_separator() {
    assert!(!looks_like_day("2025/01/15"));
}

#[test]
fn test_looks_like_day_letters() {
    assert!(!looks_like_day("abcd-ef-gh"));
}

#[test]
fn test_looks_like_day_empty() {
    assert!(!looks_like_day(""));
}

// --- classify_file_type ---

#[test]
fn test_classify_file_type_text() {
    assert_eq!(classify_file_type("text"), "text");
    assert_eq!(classify_file_type("TEXT"), "text");
    assert_eq!(classify_file_type("Text"), "text");
}

#[test]
fn test_classify_file_type_json() {
    assert_eq!(classify_file_type("json"), "json");
    assert_eq!(classify_file_type("api"), "json");
    assert_eq!(classify_file_type("anything"), "json");
}

// --- expand_tilde ---

#[test]
fn test_expand_tilde_with_home() {
    let result = expand_tilde("~/logs/claude");
    let home = std::env::var("HOME").unwrap();
    assert_eq!(result, PathBuf::from(home).join("logs/claude"));
}

#[test]
fn test_expand_tilde_no_prefix() {
    assert_eq!(expand_tilde("/absolute/path"), PathBuf::from("/absolute/path"));
}

#[test]
fn test_expand_tilde_bare_tilde() {
    assert_eq!(expand_tilde("~"), PathBuf::from("~"));
}

// --- normalize_epoch_millis ---

#[test]
fn test_normalize_epoch_millis_already_millis() {
    assert_eq!(normalize_epoch_millis(1700000000000), 1700000000000);
}

#[test]
fn test_normalize_epoch_millis_seconds() {
    assert_eq!(normalize_epoch_millis(1700000000), 1700000000000);
}

#[test]
fn test_normalize_epoch_millis_boundary() {
    assert_eq!(normalize_epoch_millis(100_000_000_000), 100_000_000_000_000);
}

#[test]
fn test_normalize_epoch_millis_above_boundary() {
    assert_eq!(normalize_epoch_millis(100_000_000_001), 100_000_000_001);
}

// --- epoch_millis_to_iso ---

#[test]
fn test_epoch_millis_to_iso_valid() {
    let result = epoch_millis_to_iso(1700000000000);
    assert_eq!(result, "2023-11-14T22:13:20.000Z");
}

#[test]
fn test_epoch_millis_to_iso_zero() {
    let result = epoch_millis_to_iso(0);
    assert_eq!(result, "1970-01-01T00:00:00.000Z");
}

// --- system_time_to_millis ---

#[test]
fn test_system_time_to_millis_epoch() {
    let epoch = SystemTime::UNIX_EPOCH;
    assert_eq!(system_time_to_millis(epoch), 0);
}

// --- system_time_to_iso ---

#[test]
fn test_system_time_to_iso_epoch() {
    let result = system_time_to_iso(SystemTime::UNIX_EPOCH);
    assert_eq!(result, "1970-01-01T00:00:00Z");
}

// --- now_iso ---

#[test]
fn test_now_iso_format() {
    let result = now_iso();
    assert!(result.ends_with('Z'));
    assert!(result.contains('T'));
    assert!(result.len() >= 24);
}

// --- parse_timestamp_value ---

#[test]
fn test_parse_timestamp_value_rfc3339() {
    let val = serde_json::json!("2023-11-14T22:13:20.000Z");
    let (ms, display) = parse_timestamp_value(&val).unwrap();
    assert_eq!(ms, 1700000000000);
    assert_eq!(display, "2023-11-14T22:13:20.000Z");
}

#[test]
fn test_parse_timestamp_value_numeric_string_seconds() {
    let val = serde_json::json!("1700000000");
    let (ms, _display) = parse_timestamp_value(&val).unwrap();
    assert_eq!(ms, 1700000000000);
}

#[test]
fn test_parse_timestamp_value_integer_seconds() {
    let val = serde_json::json!(1700000000);
    let (ms, _display) = parse_timestamp_value(&val).unwrap();
    assert_eq!(ms, 1700000000000);
}

#[test]
fn test_parse_timestamp_value_integer_millis() {
    let val = serde_json::json!(1700000000000_i64);
    let (ms, _display) = parse_timestamp_value(&val).unwrap();
    assert_eq!(ms, 1700000000000);
}

#[test]
fn test_parse_timestamp_value_garbage() {
    let val = serde_json::json!("not-a-timestamp");
    assert!(parse_timestamp_value(&val).is_none());
}

#[test]
fn test_parse_timestamp_value_null() {
    let val = serde_json::json!(null);
    assert!(parse_timestamp_value(&val).is_none());
}

// --- extract_event_timestamp ---

#[test]
fn test_extract_event_timestamp_from_timestamp_field() {
    let val = serde_json::json!({"timestamp": "2023-11-14T22:13:20.000Z"});
    let (ms, _) = extract_event_timestamp(&val).unwrap();
    assert_eq!(ms, 1700000000000);
}

#[test]
fn test_extract_event_timestamp_from_ingest_ts() {
    let val = serde_json::json!({"ingest_ts": 1700000000});
    let (ms, _) = extract_event_timestamp(&val).unwrap();
    assert_eq!(ms, 1700000000000);
}

#[test]
fn test_extract_event_timestamp_missing() {
    let val = serde_json::json!({"other": "value"});
    assert!(extract_event_timestamp(&val).is_none());
}

// --- extract_event_session ---

#[test]
fn test_extract_event_session_from_session() {
    let val = serde_json::json!({"session": "abc-123"});
    assert_eq!(extract_event_session(&val, "fallback"), "abc-123");
}

#[test]
fn test_extract_event_session_from_session_id() {
    let val = serde_json::json!({"sessionId": "xyz-789"});
    assert_eq!(extract_event_session(&val, "fallback"), "xyz-789");
}

#[test]
fn test_extract_event_session_unknown_uses_fallback() {
    let val = serde_json::json!({"session": "unknown"});
    assert_eq!(extract_event_session(&val, "fallback"), "fallback");
}

#[test]
fn test_extract_event_session_empty_uses_fallback() {
    let val = serde_json::json!({"session": ""});
    assert_eq!(extract_event_session(&val, "fallback"), "fallback");
}

#[test]
fn test_extract_event_session_missing_uses_fallback() {
    let val = serde_json::json!({});
    assert_eq!(extract_event_session(&val, "fb"), "fb");
}

// --- extract_event_type ---

#[test]
fn test_extract_event_type_present() {
    let val = serde_json::json!({"type": "tool_use"});
    assert_eq!(extract_event_type(&val, "fallback"), "tool_use");
}

#[test]
fn test_extract_event_type_empty_uses_fallback() {
    let val = serde_json::json!({"type": ""});
    assert_eq!(extract_event_type(&val, "api"), "api");
}

#[test]
fn test_extract_event_type_missing_uses_fallback() {
    let val = serde_json::json!({});
    assert_eq!(extract_event_type(&val, "api"), "api");
}

// --- extract_event_preview ---

#[test]
fn test_extract_event_preview_display() {
    let val = serde_json::json!({"display": "Hello world"});
    assert_eq!(extract_event_preview(&val), "Hello world");
}

#[test]
fn test_extract_event_preview_message_string() {
    let val = serde_json::json!({"message": "A message"});
    assert_eq!(extract_event_preview(&val), "A message");
}

#[test]
fn test_extract_event_preview_message_content_string() {
    let val = serde_json::json!({"message": {"content": "Nested content"}});
    assert_eq!(extract_event_preview(&val), "Nested content");
}

#[test]
fn test_extract_event_preview_message_content_array() {
    let val = serde_json::json!({"message": {"content": [{"text": "Array text"}]}});
    assert_eq!(extract_event_preview(&val), "Array text");
}

#[test]
fn test_extract_event_preview_text_field() {
    let val = serde_json::json!({"text": "Fallback text"});
    assert_eq!(extract_event_preview(&val), "Fallback text");
}

#[test]
fn test_extract_event_preview_none() {
    let val = serde_json::json!({"other": "data"});
    assert_eq!(extract_event_preview(&val), "(event)");
}

// --- extract_event_agent ---

#[test]
fn test_extract_event_agent_present() {
    let val = serde_json::json!({"agent": "gemini"});
    assert_eq!(extract_event_agent(&val), "gemini");
}

#[test]
fn test_extract_event_agent_empty_falls_back() {
    let val = serde_json::json!({"agent": ""});
    assert_eq!(extract_event_agent(&val), "unknown");
}

#[test]
fn test_extract_event_agent_missing_falls_back() {
    let val = serde_json::json!({"other": "value"});
    assert_eq!(extract_event_agent(&val), "unknown");
}

// --- compact_preview ---

#[test]
fn test_compact_preview_short() {
    assert_eq!(compact_preview("hello world", 50), "hello world");
}

#[test]
fn test_compact_preview_whitespace_collapse() {
    assert_eq!(compact_preview("hello   \n  world", 50), "hello world");
}

#[test]
fn test_compact_preview_empty() {
    assert_eq!(compact_preview("", 50), "(event)");
}

#[test]
fn test_compact_preview_only_whitespace() {
    assert_eq!(compact_preview("   \n\t  ", 50), "(event)");
}

#[test]
fn test_compact_preview_truncation() {
    let long = "a".repeat(200);
    let result = compact_preview(&long, 20);
    assert!(result.ends_with("..."));
    assert_eq!(result.len(), 20);
}

// --- parse_event_line ---

#[test]
fn test_parse_event_line_valid_json() {
    let ef = EventFile {
        day: "2025-01-01".to_string(),
        session: "fallback-sess".to_string(),
        channel: "api".to_string(),
        file_type: "json".to_string(),
        path: PathBuf::from("/tmp/fake"),
    };
    let line = r#"{"session":"s1","type":"tool_use","display":"hello","timestamp":"2023-11-14T22:13:20.000Z","agent":"claude"}"#;
    let parsed = parse_event_line(line, &ef).unwrap();
    assert_eq!(parsed.session, "s1");
    assert_eq!(parsed.agent, "claude");
    assert_eq!(parsed.event_type, "tool_use");
    assert_eq!(parsed.preview, "hello");
    assert_eq!(parsed.timestamp_ms, Some(1700000000000));
}

#[test]
fn test_parse_event_line_invalid_json() {
    let ef = EventFile {
        day: "2025-01-01".to_string(),
        session: "fallback".to_string(),
        channel: "api".to_string(),
        file_type: "json".to_string(),
        path: PathBuf::from("/tmp/fake"),
    };
    assert!(parse_event_line("not json", &ef).is_none());
}

#[test]
fn test_parse_event_line_no_timestamp() {
    let ef = EventFile {
        day: "2025-01-01".to_string(),
        session: "fallback".to_string(),
        channel: "api".to_string(),
        file_type: "json".to_string(),
        path: PathBuf::from("/tmp/fake"),
    };
    let line = r#"{"session":"s1","type":"tool_use","display":"hello"}"#;
    let parsed = parse_event_line(line, &ef).unwrap();
    assert_eq!(parsed.agent, "unknown");
    assert!(parsed.timestamp_ms.is_none());
    assert!(parsed.timestamp_display.is_none());
}
