use crate::model::{LogEntry, LogLevel};
use chrono::{DateTime, NaiveDateTime};
use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;

static RE_STANDARD: LazyLock<Regex> = LazyLock::new(|| {
    // Matches: 2026-09-05 12:34:56.789 [INFO] [auth] Message
    // Matches: [2026-09-05T12:34:56.789Z] [WARN] [api] Message
    // Matches: 2026-09-05 12:34:56 ERROR: Something went wrong
    Regex::new(r#"^(?:\[?(\d{4}[-/]\d{2}[-/]\d{2}[T ]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})?)\]?\s+)?(?:\[?([A-Za-z]{3,7})\]?:?\s+)?(?:\[([a-zA-Z0-9_\-.]+)\]:?\s+)?(.*)$"#)
        .unwrap()
});

static RE_LOGCAT: LazyLock<Regex> = LazyLock::new(|| {
    // 09-05 12:34:56.789  1234  5678 E AndroidRuntime: Message
    Regex::new(r#"^(\d{2}-\d{2}\s+\d{2}:\d{2}:\d{2}\.\d+)\s+\d+\s+\d+\s+([VDIWEF])\s+([^:]+):\s+(.*)$"#)
        .unwrap()
});

static RE_SYSLOG: LazyLock<Regex> = LazyLock::new(|| {
    // Sep  5 12:34:56 hostname process[1234]: Message
    Regex::new(r#"^([A-Z][a-z]{2}\s+\d+\s+\d{2}:\d{2}:\d{2})\s+([a-zA-Z0-9_\-]+)\s+([a-zA-Z0-9_\-]+)(?:\[\d+\])?:\s+(.*)$"#)
        .unwrap()
});

pub fn parse_text_bytes(bytes: &[u8]) -> anyhow::Result<Vec<LogEntry>> {
    let content = String::from_utf8_lossy(bytes);
    let mut entries = Vec::new();
    let mut prev_ts = 0.0;

    for (i, line) in content.lines().enumerate() {
        let index = i + 1;
        let line_trimmed = line.trim();
        if line_trimmed.is_empty() {
            continue;
        }

        // Try parsing JSON log line first
        if line_trimmed.starts_with('{') && line_trimmed.ends_with('}') {
            if let Ok(json_val) = serde_json::from_str::<Value>(line_trimmed) {
                if let Some(entry) = parse_json_log_entry(index, &json_val, line, prev_ts) {
                    prev_ts = entry.timestamp;
                    entries.push(entry);
                    continue;
                }
            }
        }

        // Try Logcat format
        if let Some(caps) = RE_LOGCAT.captures(line_trimmed) {
            let time_str = caps.get(1).map_or("", |m| m.as_str()).to_string();
            let level_code = caps.get(2).map_or("I", |m| m.as_str());
            let tag = caps.get(3).map_or("app", |m| m.as_str()).to_string();
            let payload = caps.get(4).map_or("", |m| m.as_str()).to_string();

            let level = match level_code {
                "F" => LogLevel::Fatal,
                "E" => LogLevel::Error,
                "W" => LogLevel::Warn,
                "I" => LogLevel::Info,
                "D" => LogLevel::Debug,
                "V" => LogLevel::Verbose,
                _ => LogLevel::Unknown,
            };

            let timestamp = prev_ts + 0.001;
            prev_ts = timestamp;
            entries.push(LogEntry::new(index, timestamp, time_str, tag, level, payload, line.to_string()));
            continue;
        }

        // Try Syslog format
        if let Some(caps) = RE_SYSLOG.captures(line_trimmed) {
            let time_str = caps.get(1).map_or("", |m| m.as_str()).to_string();
            let tag = caps.get(3).map_or("sys", |m| m.as_str()).to_string();
            let payload = caps.get(4).map_or("", |m| m.as_str()).to_string();
            let level = LogLevel::from_str(&payload);

            let timestamp = prev_ts + 0.001;
            prev_ts = timestamp;
            entries.push(LogEntry::new(index, timestamp, time_str, tag, level, payload, line.to_string()));
            continue;
        }

        // Try Standard timestamped / tagged format
        let mut timestamp = prev_ts + 0.001;
        let mut time_str = format!("{:.3}", timestamp);
        let mut tag = "app".to_string();
        let mut payload = line.to_string();
        let level;

        if let Some(caps) = RE_STANDARD.captures(line_trimmed) {
            // Timestamp
            if let Some(ts_match) = caps.get(1) {
                let s = ts_match.as_str();
                time_str = s.to_string();
                if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
                    timestamp = dt.timestamp() as f64 + (dt.timestamp_subsec_micros() as f64 / 1_000_000.0);
                } else if let Ok(ndt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f") {
                    timestamp = ndt.and_utc().timestamp() as f64 + (ndt.and_utc().timestamp_subsec_micros() as f64 / 1_000_000.0);
                } else if let Ok(ndt) = NaiveDateTime::parse_from_str(s, "%Y/%m/%d %H:%M:%S") {
                    timestamp = ndt.and_utc().timestamp() as f64;
                }
            }

            // Level
            if let Some(lvl_match) = caps.get(2) {
                level = LogLevel::from_str(lvl_match.as_str());
            } else {
                level = LogLevel::from_str(line);
            }

            // Tag
            if let Some(tag_match) = caps.get(3) {
                tag = tag_match.as_str().to_string();
            }

            // Payload
            if let Some(p_match) = caps.get(4) {
                let p = p_match.as_str().trim();
                if !p.is_empty() {
                    payload = p.to_string();
                }
            }
        } else {
            level = LogLevel::from_str(line);
        }

        prev_ts = timestamp;
        entries.push(LogEntry::new(index, timestamp, time_str, tag, level, payload, line.to_string()));
    }

    Ok(entries)
}

fn parse_json_log_entry(index: usize, json: &Value, raw_line: &str, prev_ts: f64) -> Option<LogEntry> {
    let mut timestamp = prev_ts + 0.001;
    let mut time_str = format!("{:.3}", timestamp);

    // Extract timestamp
    for key in &["timestamp", "time", "@timestamp", "datetime", "date"] {
        if let Some(val) = json.get(*key).and_then(|v| v.as_str()) {
            time_str = val.to_string();
            if let Ok(dt) = DateTime::parse_from_rfc3339(val) {
                timestamp = dt.timestamp() as f64 + (dt.timestamp_subsec_micros() as f64 / 1_000_000.0);
            }
            break;
        }
    }

    // Extract level
    let mut level = LogLevel::Info;
    for key in &["level", "severity", "loglevel", "log_level"] {
        if let Some(val) = json.get(*key).and_then(|v| v.as_str()) {
            level = LogLevel::from_str(val);
            break;
        }
    }

    // Extract tag/service/component
    let mut tag = "app".to_string();
    for key in &["service", "tag", "logger", "module", "component", "caller"] {
        if let Some(val) = json.get(*key).and_then(|v| v.as_str()) {
            tag = val.to_string();
            break;
        }
    }

    // Extract payload/message
    let mut payload = raw_line.to_string();
    for key in &["message", "msg", "log", "payload", "text"] {
        if let Some(val) = json.get(*key).and_then(|v| v.as_str()) {
            payload = val.to_string();
            break;
        }
    }

    Some(LogEntry::new(index, timestamp, time_str, tag, level, payload, raw_line.to_string()))
}
