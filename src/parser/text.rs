use crate::model::{LogEntry, LogLevel};
use crate::parser::event::{json_to_log_value, LogEvent, LogValue};
use crate::query::schema::{FieldType, LogSchema, QueryField};
use chrono::{DateTime, NaiveDateTime};
use regex::Regex;
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};
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

/// How many events to sample when inferring a per-file `LogSchema`.
const SCHEMA_SAMPLE_LIMIT: usize = 1000;
/// An enumerable string field must have at most this many distinct values.
const ENUM_CARDINALITY_CAP: usize = 20;

#[derive(Debug, Clone)]
pub struct ParseResult {
    pub entries: Vec<LogEntry>,
    pub max_chars: crate::column::FieldMaxChars,
}

#[derive(Debug, Clone)]
pub struct SchemaParseResult {
    pub schema: LogSchema,
    pub events: Vec<LogEvent>,
    pub max_chars: crate::column::FieldMaxChars,
}

pub fn parse_text_bytes(bytes: &[u8]) -> anyhow::Result<Vec<LogEntry>> {
    parse_text_bytes_with_max_chars(bytes).map(|res| res.entries)
}

pub fn parse_text_bytes_with_max_chars(bytes: &[u8]) -> anyhow::Result<ParseResult> {
    let content = String::from_utf8_lossy(bytes);
    let mut entries = Vec::new();
    let mut max_chars = crate::column::FieldMaxChars::new();

    for (i, line) in content.lines().enumerate() {
        let index = i + 1;
        let line_trimmed = line.trim();
        if line_trimmed.is_empty() {
            continue;
        }
        if let Some(event) = parse_line(index, line_trimmed, line) {
            let entry = LogEntry::from_event(&event);
            max_chars.record_entry(&entry);
            entries.push(entry);
        }
    }

    Ok(ParseResult { entries, max_chars })
}

/// Parse bytes into structured events plus a per-file schema inferred from
/// the discovered fields.
pub fn parse_text_bytes_with_schema(bytes: &[u8]) -> anyhow::Result<SchemaParseResult> {
    let content = String::from_utf8_lossy(bytes);
    let mut events = Vec::new();
    let mut max_chars = crate::column::FieldMaxChars::new();
    let mut schema_builder = SchemaBuilder::new();

    for (i, line) in content.lines().enumerate() {
        let index = i + 1;
        let line_trimmed = line.trim();
        if line_trimmed.is_empty() {
            continue;
        }
        if let Some(event) = parse_line(index, line_trimmed, line) {
            schema_builder.record_event(&event);
            let entry = LogEntry::from_event(&event);
            max_chars.record_entry(&entry);
            events.push(event);
        }
    }

    Ok(SchemaParseResult {
        schema: schema_builder.build(),
        events,
        max_chars,
    })
}

/// Detect the format of a single line and produce a structured `LogEvent`.
fn parse_line(index: usize, line_trimmed: &str, raw_line: &str) -> Option<LogEvent> {
    // Try parsing JSON log line first
    if line_trimmed.starts_with('{') && line_trimmed.ends_with('}') {
        if let Ok(json_val) = serde_json::from_str::<Value>(line_trimmed) {
            if let Some(event) = parse_json_log_event(index, &json_val, raw_line) {
                return Some(event);
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

        let timestamp = infer_incrementing_timestamp(index);
        return Some(
            LogEvent::new(index, timestamp, time_str, payload, raw_line.to_string())
                .with_metadata(level, &tag),
        );
    }

    // Try Syslog format
    if let Some(caps) = RE_SYSLOG.captures(line_trimmed) {
        let time_str = caps.get(1).map_or("", |m| m.as_str()).to_string();
        let tag = caps.get(3).map_or("sys", |m| m.as_str()).to_string();
        let payload = caps.get(4).map_or("", |m| m.as_str()).to_string();
        let level = LogLevel::from_str(&payload);

        let timestamp = infer_incrementing_timestamp(index);
        return Some(
            LogEvent::new(index, timestamp, time_str, payload, raw_line.to_string())
                .with_metadata(level, &tag),
        );
    }

    // Try Standard timestamped / tagged format
    let mut timestamp = infer_incrementing_timestamp(index);
    let mut time_str = format!("{:.3}", timestamp);
    let mut tag = "app".to_string();
    let mut payload = raw_line.to_string();

    if let Some(caps) = RE_STANDARD.captures(line_trimmed) {
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

        if let Some(tag_match) = caps.get(3) {
            tag = tag_match.as_str().to_string();
        }

        if let Some(p_match) = caps.get(4) {
            let p = p_match.as_str().trim();
            if !p.is_empty() {
                payload = p.to_string();
            }
        }
    }

    let level = LogLevel::from_str(line_trimmed);
    Some(
        LogEvent::new(index, timestamp, time_str, payload, raw_line.to_string())
            .with_metadata(level, &tag),
    )
}

/// Deterministic pseudo-timestamp for formats without a parseable timestamp.
fn infer_incrementing_timestamp(index: usize) -> f64 {
    index as f64 * 0.001
}

fn parse_json_log_event(
    index: usize,
    json: &Value,
    raw_line: &str,
) -> Option<LogEvent> {
    let mut timestamp = infer_incrementing_timestamp(index);
    let mut time_str = format!("{:.3}", timestamp);

    for key in &["timestamp", "time", "@timestamp", "datetime", "date"] {
        if let Some(val) = json.get(*key).and_then(|v| v.as_str()) {
            time_str = val.to_string();
            if let Ok(dt) = DateTime::parse_from_rfc3339(val) {
                timestamp = dt.timestamp() as f64 + (dt.timestamp_subsec_micros() as f64 / 1_000_000.0);
            }
            break;
        }
    }

    let mut level = LogLevel::Info;
    for key in &["level", "severity", "loglevel", "log_level"] {
        if let Some(val) = json.get(*key).and_then(|v| v.as_str()) {
            level = LogLevel::from_str(val);
            break;
        }
    }

    let mut tag = "app".to_string();
    let mut tag_key: Option<&str> = None;
    for key in &["service", "tag", "logger", "module", "component", "caller"] {
        if let Some(val) = json.get(*key).and_then(|v| v.as_str()) {
            tag = val.to_string();
            tag_key = Some(key);
            break;
        }
    }

    let mut payload = raw_line.to_string();
    for key in &["message", "msg", "log", "payload", "text"] {
        if let Some(val) = json.get(*key).and_then(|v| v.as_str()) {
            payload = val.to_string();
            break;
        }
    }

    let mut event = LogEvent::new(index, timestamp, time_str, payload.clone(), raw_line.to_string())
        .with_metadata(level, &tag);

    // Insert every raw JSON key as a dynamic field.
    if let Value::Object(map) = json {
        for (key, value) in map {
            event.set_field(key, json_to_log_value(value));
        }
    }

    // Ensure canonical `module`/`message` fields exist even when the source
    // JSON used a different key name.
    if tag_key.is_some() && !json.get("module").is_some() {
        event.set_field("module", LogValue::Str(tag.clone()));
    }
    if !json.get("message").is_some() && !json.get("msg").is_some() {
        event.set_field("message", LogValue::Str(payload.clone()));
    }
    event.set_field("level", LogValue::Str(level.as_str().to_string()));
    event.set_field("time", LogValue::Str(event.time_str.clone()));

    Some(event)
}

/// Incremental type discovery used to infer a per-file `LogSchema`.
struct TypeAccum {
    saw_string: bool,
    saw_number: bool,
    saw_bool: bool,
    is_json: bool,
    occurrences: usize,
    enum_values: Option<BTreeSet<String>>,
}

impl Default for TypeAccum {
    fn default() -> Self {
        Self {
            saw_string: false,
            saw_number: false,
            saw_bool: false,
            is_json: false,
            occurrences: 0,
            enum_values: Some(BTreeSet::new()),
        }
    }
}

struct SchemaBuilder {
    accums: HashMap<String, TypeAccum>,
    events_seen: usize,
}

impl SchemaBuilder {
    fn new() -> Self {
        Self {
            accums: HashMap::new(),
            events_seen: 0,
        }
    }

    fn record_event(&mut self, event: &LogEvent) {
        if self.events_seen >= SCHEMA_SAMPLE_LIMIT {
            return;
        }
        self.events_seen += 1;
        for (name, value) in &event.fields {
            let accum = self.accums.entry(name.clone()).or_default();
            match value {
                LogValue::Str(s) => {
                    accum.saw_string = true;
                    accum.occurrences += 1;
                    if let Some(set) = &mut accum.enum_values {
                        if set.len() < ENUM_CARDINALITY_CAP {
                            set.insert(s.clone());
                        } else if !set.contains(s) {
                            accum.enum_values = None;
                        }
                    }
                }
                LogValue::Number(_) => {
                    accum.saw_number = true;
                    accum.occurrences += 1;
                }
                LogValue::Bool(_) => {
                    accum.saw_bool = true;
                    accum.occurrences += 1;
                }
                LogValue::Null => {}
            }
        }
    }

    fn build(mut self) -> LogSchema {
        let accums: Vec<(String, TypeAccum)> = self.accums.drain().collect();
        let mut fields = Vec::new();
        for (name, acc) in accums {
            let field_type = self.infer_type(&name, &acc);
            let mut field = QueryField::new(&name, field_type);
            if field_type == FieldType::Enum {
                if let Some(values) = acc.enum_values {
                    field.enum_values = Some(values.into_iter().collect());
                }
            }
            fields.push(field);
        }
        fields.sort_by(|a, b| a.name.cmp(&b.name));
        LogSchema { fields }
    }

    fn infer_type(&self, name: &str, acc: &TypeAccum) -> FieldType {
        // Canonical time fields are always timestamps.
        if matches!(name, "time" | "timestamp" | "@timestamp" | "datetime" | "date") {
            return FieldType::Timestamp;
        }
        if acc.is_json {
            return FieldType::Json;
        }
        match (acc.saw_string, acc.saw_number, acc.saw_bool) {
            (_, _, _) if acc.saw_number && !acc.saw_string && !acc.saw_bool => FieldType::Number,
            (_, _, _) if acc.saw_bool && !acc.saw_string && !acc.saw_number => FieldType::Boolean,
            (_, _, _) if acc.saw_string && !acc.saw_number && !acc.saw_bool => {
                // Fields with repeated low-cardinality values are enumerable; the
                // canonical level/module fields always are (when pure strings).
                if let Some(set) = &acc.enum_values {
                    let distinct = set.len();
                    if distinct > 0
                        && (matches!(name, "level" | "module") || distinct < acc.occurrences)
                    {
                        return FieldType::Enum;
                    }
                }
                FieldType::String
            }
            (true, _, _) => FieldType::String,
            _ => FieldType::String,
        }
    }
}