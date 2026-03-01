use std::path::PathBuf;
use std::time::SystemTime;

use chrono::{DateTime, TimeZone, Utc};

use crate::models::{EventFile, HookInfo, ParsedEventLine, TokenUsage};

pub fn looks_like_day(value: &str) -> bool {
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

pub fn classify_file_type(channel_name: &str) -> String {
    if channel_name.eq_ignore_ascii_case("text") {
        "text".to_string()
    } else {
        "json".to_string()
    }
}

pub fn expand_tilde(input: &str) -> PathBuf {
    if let Some(stripped) = input.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(stripped);
        }
    }
    PathBuf::from(input)
}

pub fn normalize_epoch_millis(raw: i64) -> i64 {
    if raw > 100_000_000_000 {
        raw
    } else {
        raw.saturating_mul(1000)
    }
}

pub fn epoch_millis_to_iso(ms: i64) -> String {
    Utc.timestamp_millis_opt(ms)
        .single()
        .map(|ts| ts.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
        .unwrap_or_else(|| "n/a".to_string())
}

pub fn system_time_to_millis(value: SystemTime) -> i64 {
    DateTime::<Utc>::from(value).timestamp_millis()
}

pub fn system_time_to_iso(value: SystemTime) -> String {
    DateTime::<Utc>::from(value).to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

pub fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub fn parse_timestamp_value(value: &serde_json::Value) -> Option<(i64, String)> {
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

pub fn extract_event_timestamp(value: &serde_json::Value) -> Option<(i64, String)> {
    for key in ["timestamp", "ingest_ts"] {
        if let Some(raw) = value.get(key) {
            if let Some((sort_key_ms, display)) = parse_timestamp_value(raw) {
                return Some((sort_key_ms, display));
            }
        }
    }
    None
}

pub fn extract_event_session(value: &serde_json::Value, fallback_session: &str) -> String {
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

pub fn extract_event_type(value: &serde_json::Value, fallback_channel: &str) -> String {
    value
        .get("type")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback_channel)
        .to_string()
}

pub fn extract_event_agent(value: &serde_json::Value) -> String {
    value
        .get("agent")
        .and_then(serde_json::Value::as_str)
        .filter(|v| !v.is_empty())
        .unwrap_or("unknown")
        .to_string()
}

pub fn extract_event_preview(value: &serde_json::Value) -> String {
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

pub fn compact_preview(value: &str, max_chars: usize) -> String {
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
    format!("{short}...")
}

pub fn extract_token_usage(value: &serde_json::Value) -> Option<TokenUsage> {
    let usage = value.get("message")?.get("usage")?;
    Some(TokenUsage {
        input_tokens: usage.get("input_tokens").and_then(|v| v.as_i64()).unwrap_or(0),
        output_tokens: usage.get("output_tokens").and_then(|v| v.as_i64()).unwrap_or(0),
        cache_read_input_tokens: usage
            .get("cache_read_input_tokens")
            .and_then(|v| v.as_i64())
            .unwrap_or(0),
        cache_creation_input_tokens: usage
            .get("cache_creation_input_tokens")
            .and_then(|v| v.as_i64())
            .unwrap_or(0),
    })
}

pub fn extract_model(value: &serde_json::Value) -> Option<String> {
    value
        .get("message")?
        .get("model")?
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

pub fn extract_tool_uses(value: &serde_json::Value) -> Vec<String> {
    let content = match value.get("message").and_then(|m| m.get("content")).and_then(|c| c.as_array()) {
        Some(arr) => arr,
        None => return Vec::new(),
    };
    content
        .iter()
        .filter(|item| item.get("type").and_then(|t| t.as_str()) == Some("tool_use"))
        .filter_map(|item| item.get("name").and_then(|n| n.as_str()).map(|s| s.to_string()))
        .collect()
}

pub fn extract_turn_duration(value: &serde_json::Value) -> Option<i64> {
    if value.get("subtype").and_then(|s| s.as_str()) != Some("turn_duration") {
        return None;
    }
    value.get("durationMs").and_then(|v| v.as_i64())
}

pub fn extract_hook_infos(value: &serde_json::Value) -> Vec<HookInfo> {
    if value.get("subtype").and_then(|s| s.as_str()) != Some("stop_hook_summary") {
        return Vec::new();
    }
    let infos = match value.get("hookInfos").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return Vec::new(),
    };
    infos
        .iter()
        .filter_map(|item| {
            let command = item.get("command").and_then(|c| c.as_str())?.to_string();
            let duration_ms = item.get("durationMs").and_then(|d| d.as_i64()).unwrap_or(0);
            Some(HookInfo {
                command,
                duration_ms,
            })
        })
        .collect()
}

pub fn extract_is_api_error(value: &serde_json::Value) -> bool {
    value
        .get("isApiErrorMessage")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

pub fn extract_is_tool_error(value: &serde_json::Value) -> bool {
    let content = match value.get("message").and_then(|m| m.get("content")).and_then(|c| c.as_array()) {
        Some(arr) => arr,
        None => return false,
    };
    content.iter().any(|item| {
        item.get("type").and_then(|t| t.as_str()) == Some("tool_result")
            && item.get("is_error").and_then(|v| v.as_bool()).unwrap_or(false)
    })
}

pub fn parse_event_line(line: &str, event_file: &EventFile) -> Option<ParsedEventLine> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    let timestamp = extract_event_timestamp(&value);
    let (timestamp_ms, timestamp_display) = match timestamp {
        Some((ms, display)) => (Some(ms), Some(display)),
        None => (None, None),
    };
    let session = extract_event_session(&value, &event_file.session);
    let agent = extract_event_agent(&value);
    let event_type = extract_event_type(&value, &event_file.channel);
    let preview = extract_event_preview(&value);
    let token_usage = extract_token_usage(&value);
    let model = extract_model(&value);
    let tool_uses = extract_tool_uses(&value);
    let turn_duration_ms = extract_turn_duration(&value);
    let hook_infos = extract_hook_infos(&value);
    let is_api_error = extract_is_api_error(&value);
    let is_tool_error = extract_is_tool_error(&value);

    Some(ParsedEventLine {
        session,
        agent,
        event_type,
        preview,
        timestamp_ms,
        timestamp_display,
        token_usage,
        model,
        tool_uses,
        turn_duration_ms,
        hook_infos,
        is_api_error,
        is_tool_error,
    })
}

