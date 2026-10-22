use crate::model::{LogEntry, LogLevel};
use crate::parser::event::{json_to_log_value, LogEvent, LogValue, SpanList};
use crate::parser::schema::{FieldType, LogSchema, QueryField};
use chrono::{DateTime, NaiveDateTime};
use regex::Regex;
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};
use std::ops::Range;
use std::sync::{Arc, LazyLock};

pub use crate::extract::LineConfig;
use crate::log_format::CompiledFormat;

static RE_STANDARD: LazyLock<Regex> = LazyLock::new(|| {
    // Matches: 2026-09-05 12:34:56.789 [INFO] [auth] Message
    // Matches: [2026-09-05T12:34:56.789Z] [WARN] [api] Message
    // Matches: 2026-09-05 12:34:56 ERROR: Something went wrong
    Regex::new(r#"^(?:\[?(\d{4}[-/]\d{2}[-/]\d{2}[T ]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})?)\]?\s+)?(?:\[?([A-Za-z]{1,7})\]?:?\s+)?(?:\[([a-zA-Z0-9_\-.]+)\]:?\s+)?(.*)$"#)
        .unwrap()
});

static RE_STANDARD_NO_LEVEL: LazyLock<Regex> = LazyLock::new(|| {
    // The same line when the word after the timestamp is not a level.
    Regex::new(r#"^(?:\[?(\d{4}[-/]\d{2}[-/]\d{2}[T ]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})?)\]?\s+)?(?:\[([a-zA-Z0-9_\-.]+)\]:?\s+)?(.*)$"#)
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
    parse_text_bytes_with_config(bytes, &LineConfig::default())
}

/// Parse bytes, reading the fields of `config` from every line as well.
pub fn parse_text_bytes_with_config(bytes: &[u8], config: &LineConfig) -> anyhow::Result<ParseResult> {
    let content = String::from_utf8_lossy(bytes);
    let mut entries = parse_entries(&content);
    config.apply(&mut entries);
    let max_chars = crate::column::FieldMaxChars::scan_entries(&entries);
    Ok(ParseResult { entries, max_chars })
}

/// Parse bytes into structured events plus a per-file schema inferred from
/// the discovered fields.
pub fn parse_text_bytes_with_schema(bytes: &[u8]) -> anyhow::Result<SchemaParseResult> {
    let content = String::from_utf8_lossy(bytes);
    let mut events = Vec::new();
    let mut max_chars = crate::column::FieldMaxChars::new();
    let mut schema_builder = SchemaBuilder::new();

    for event in parse_events(&content) {
        schema_builder.record_event(&event);
        let entry = LogEntry::from_event(&event);
        max_chars.record_entry(&entry);
        events.push(event);
    }

    Ok(SchemaParseResult {
        schema: schema_builder.build(),
        events,
        max_chars,
    })
}

/// Parse every stored line (empty ones included) into one event, then mark
/// the Continuation Lines: a line without a time of its own that follows a
/// line with one. Its Parent Line is the nearest line above with a time.
/// Lines before the first timestamp have no Parent Line.
pub fn parse_events(content: &str) -> Vec<LogEvent> {
    parse_events_with(content, None)
}

/// The same, reading the lines `format` matches with it.
pub fn parse_events_with(content: &str, format: Option<&CompiledFormat>) -> Vec<LogEvent> {
    let mut events = Vec::new();
    let mut parent: Option<usize> = None;
    for (i, line) in content.lines().enumerate() {
        let index = i + 1;
        let mut event = parse_line(index, line, format);
        if event.has_time {
            parent = Some(index);
        } else if let Some(p) = parent {
            event = event.into_continuation(p);
        }
        events.push(event);
    }
    events
}

/// The entries of every stored line, read as `parse_events` reads them, one
/// line at a time: the events of a million lines are never held together.
pub fn parse_entries(content: &str) -> Vec<LogEntry> {
    let mut entries = Vec::with_capacity(content.bytes().filter(|b| *b == b'\n').count() + 1);
    let mut parent: Option<usize> = None;
    for (i, line) in content.lines().enumerate() {
        let index = i + 1;
        let mut event = parse_line(index, line, None);
        if event.has_time {
            parent = Some(index);
        } else if let Some(p) = parent {
            event = event.into_continuation(p);
        }
        entries.push(LogEntry::from_event(&event));
    }
    entries
}

/// Parses lines one at a time as they arrive, the way `parse_events` parses a
/// whole file: a line without a time of its own continues the nearest line
/// above that has one.
#[derive(Debug, Clone, Default)]
pub struct LineParser {
    parent: Option<usize>,
    /// Fields read beyond Auto-detect (Detected Fields).
    config: Arc<LineConfig>,
    /// The Log Format of the file, if it has one.
    format: Option<Arc<CompiledFormat>>,
}

impl LineParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Read lines with `config` from now on.
    pub fn set_config(&mut self, config: Arc<LineConfig>) {
        self.config = config;
    }

    /// Read lines with the Log Format `format` from now on (`None`:
    /// Auto-detect).
    pub fn set_format(&mut self, format: Option<Arc<CompiledFormat>>) {
        self.format = format;
    }

    pub fn format(&self) -> Option<Arc<CompiledFormat>> {
        self.format.clone()
    }

    /// Forget the Parent Line: what comes next starts a new file.
    pub fn reset(&mut self) {
        self.parent = None;
    }

    /// The line number of the Parent Line the next Continuation Line goes
    /// under.
    pub fn parent(&self) -> Option<usize> {
        self.parent
    }

    pub fn set_parent(&mut self, parent: Option<usize>) {
        self.parent = parent;
    }

    /// Whether `other` reads lines the same way: the same Detected Fields
    /// and the same Log Format (the same ones, not equal ones).
    pub fn reads_like(&self, other: &LineParser) -> bool {
        Arc::ptr_eq(&self.config, &other.config)
            && match (&self.format, &other.format) {
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            }
    }

    /// The entry for line number `index`.
    pub fn parse(&mut self, index: usize, line: &str) -> LogEntry {
        let mut event = parse_line(index, line, self.format.as_deref());
        if event.has_time {
            self.parent = Some(index);
        } else if let Some(p) = self.parent {
            event = event.into_continuation(p);
        }
        let mut entry = LogEntry::from_event(&event);
        entry.extra = self.config.extra_spans(&entry);
        entry
    }
}

/// Byte range of a regex group, shifted by the leading whitespace that was
/// trimmed off the line before matching.
fn group_range(caps: &regex::Captures, i: usize, lead: usize) -> Option<Range<usize>> {
    caps.get(i).map(|m| m.start() + lead..m.end() + lead)
}

/// A line read with a Log Format, or `None` when the format does not match
/// it. Its `timestamp` field is read like the time Auto-detect finds, so the
/// line sorts, filters by time and starts Continuation Lines the same way.
fn parse_with_format(index: usize, raw_line: &str, format: &CompiledFormat) -> Option<LogEvent> {
    let mut spans = format.read(raw_line)?;
    spans.retain(|_, r| r.start < r.end);
    let text = |name: &str| spans.get(name).map(|r| &raw_line[r.clone()]);
    let time_str = text("timestamp").unwrap_or_default().to_string();
    let parsed = text("timestamp").and_then(read_timestamp);
    let message = text("message").unwrap_or(raw_line).to_string();
    let mut event = LogEvent::new(index, parsed.unwrap_or_else(|| infer_incrementing_timestamp(index)), time_str, message, raw_line.to_string())
        .with_metadata(text("level"), text("module"));
    for (name, range) in &spans {
        if !matches!(name.as_str(), "timestamp" | "level" | "module" | "message") {
            event.set_field(name, LogValue::Str(raw_line[range.clone()].to_string()));
        }
    }
    if !spans.contains_key("message") {
        spans.insert("message".to_string(), 0..raw_line.len());
    }
    event.spans = SpanList::from_map(&spans);
    Some(if parsed.is_some() { event } else { event.without_time() })
}

/// Read one line: with its Log Format when the format matches it, else by
/// Auto-detect.
pub fn parse_line_with(index: usize, raw_line: &str, format: Option<&CompiledFormat>) -> LogEvent {
    parse_line(index, raw_line, format)
}

/// Detect the format of a single line and produce a structured `LogEvent`.
/// What a cell shows is always a span of `raw_line`; nothing is made up.
fn parse_line(index: usize, raw_line: &str, format: Option<&CompiledFormat>) -> LogEvent {
    if let Some(event) = format.and_then(|f| parse_with_format(index, raw_line, f)) {
        return event;
    }
    let line_trimmed = raw_line.trim();
    let lead = raw_line.len() - raw_line.trim_start().len();
    let whole = 0..raw_line.len();

    if line_trimmed.is_empty() {
        return LogEvent::new(index, infer_incrementing_timestamp(index), String::new(), String::new(), raw_line.to_string())
            .without_time()
            .with_span("message", Some(whole));
    }

    // Try parsing JSON log line first
    if line_trimmed.starts_with('{') && line_trimmed.ends_with('}') {
        if let Ok(json_val) = serde_json::from_str::<Value>(line_trimmed) {
            if let Some(event) = parse_json_log_event(index, &json_val, raw_line) {
                return event;
            }
        }
    }

    // Try Logcat format
    if let Some(caps) = RE_LOGCAT.captures(line_trimmed) {
        let time_str = caps.get(1).map_or("", |m| m.as_str()).to_string();
        let level_text = caps.get(2).map_or("", |m| m.as_str());
        let tag = caps.get(3).map_or("", |m| m.as_str());
        let payload = caps.get(4).map_or("", |m| m.as_str()).to_string();

        // The format has no year: read it in a fixed one so files compare.
        let parsed = NaiveDateTime::parse_from_str(&format!("{PLACEHOLDER_YEAR}-{time_str}"), "%Y-%m-%d %H:%M:%S%.f")
            .ok()
            .map(|ndt| ndt.and_utc().timestamp() as f64 + (ndt.and_utc().timestamp_subsec_micros() as f64 / 1_000_000.0));
        let timestamp = parsed.unwrap_or_else(|| infer_incrementing_timestamp(index));
        let event = LogEvent::new(index, timestamp, time_str, payload, raw_line.to_string())
            .with_metadata(Some(level_text), Some(tag))
            .with_span("timestamp", group_range(&caps, 1, lead))
            .with_span("level", group_range(&caps, 2, lead))
            .with_span("module", group_range(&caps, 3, lead))
            .with_span("message", group_range(&caps, 4, lead));
        return if parsed.is_some() { event } else { event.without_time() };
    }

    // Try Syslog format
    if let Some(caps) = RE_SYSLOG.captures(line_trimmed) {
        let time_str = caps.get(1).map_or("", |m| m.as_str()).to_string();
        let tag = caps.get(3).map_or("", |m| m.as_str());
        let payload = caps.get(4).map_or("", |m| m.as_str()).to_string();

        let parsed = NaiveDateTime::parse_from_str(&format!("{PLACEHOLDER_YEAR} {time_str}"), "%Y %b %e %H:%M:%S")
            .ok()
            .map(|ndt| ndt.and_utc().timestamp() as f64);
        let timestamp = parsed.unwrap_or_else(|| infer_incrementing_timestamp(index));
        let event = LogEvent::new(index, timestamp, time_str, payload, raw_line.to_string())
            .with_metadata(None, Some(tag))
            .with_span("timestamp", group_range(&caps, 1, lead))
            .with_span("module", group_range(&caps, 3, lead))
            .with_span("message", group_range(&caps, 4, lead));
        return if parsed.is_some() { event } else { event.without_time() };
    }

    // Standard timestamped / tagged format. The level is only a level when
    // the word is one; otherwise the word belongs to the message.
    let with_level = RE_STANDARD.captures(line_trimmed).filter(|caps| {
        // A lone letter (`E`) is only a level after a timestamp: "I think"
        // at the start of a line is a sentence.
        caps.get(2).is_none_or(|m| {
            LogLevel::from_token(m.as_str()) != LogLevel::Unknown
                && (m.as_str().len() > 1 || caps.get(1).is_some())
        })
    });
    let (caps, level_group, tag_group, msg_group) = match with_level {
        Some(caps) => (caps, Some(2), 3, 4),
        None => (
            RE_STANDARD_NO_LEVEL
                .captures(line_trimmed)
                .expect("RE_STANDARD_NO_LEVEL matches every line"),
            None,
            2,
            3,
        ),
    };
    let ts_group = 1;

    let mut timestamp = infer_incrementing_timestamp(index);
    let mut has_time = false;
    let mut time_str = String::new();
    if let Some(ts_match) = caps.get(ts_group) {
        let s = ts_match.as_str();
        time_str = s.to_string();
        if let Some(t) = read_timestamp(s) {
            timestamp = t;
            has_time = true;
        }
    }

    let level_range = level_group.and_then(|g| group_range(&caps, g, lead));
    let tag_range = group_range(&caps, tag_group, lead);
    let mut msg_range = group_range(&caps, msg_group, lead).filter(|r| !r.is_empty());
    // A line with nothing recognised in it is all message, as written.
    if caps.get(ts_group).is_none() && level_range.is_none() && tag_range.is_none() {
        msg_range = Some(0..raw_line.len());
    }
    let msg_range = msg_range.unwrap_or(0..raw_line.len());
    let level_text = level_range.as_ref().map(|r| &raw_line[r.clone()]);
    let tag = tag_range.as_ref().map(|r| &raw_line[r.clone()]);
    let payload = raw_line[msg_range.clone()].to_string();

    let event = LogEvent::new(index, timestamp, time_str, payload, raw_line.to_string())
        .with_metadata(level_text, tag)
        .with_span("timestamp", group_range(&caps, ts_group, lead))
        .with_span("level", level_range)
        .with_span("module", tag_range)
        .with_span("message", Some(msg_range));
    if has_time { event } else { event.without_time() }
}

/// Year assumed for formats that carry no year (logcat, syslog).
const PLACEHOLDER_YEAR: i32 = 1972;

static RE_TZ_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:Z|[+-]\d{2}:?\d{2})$").unwrap());

fn clock_seconds(ndt: NaiveDateTime) -> f64 {
    let utc = ndt.and_utc();
    utc.timestamp() as f64 + f64::from(utc.timestamp_subsec_micros()) / 1_000_000.0
}

/// Read a time as written in a log line, on the file's own clock: a zone
/// suffix is dropped, never converted, so files compare as written. Knows
/// every layout the parser reads (ISO-like, logcat, syslog).
pub fn read_timestamp(text: &str) -> Option<f64> {
    let t = text.trim();
    if let Ok(dt) = DateTime::parse_from_rfc3339(t) {
        return Some(clock_seconds(dt.naive_local()));
    }
    let bare = RE_TZ_SUFFIX.replace(t, "");
    // Python's logging writes the fraction after a comma.
    let bare = bare.trim_end().replace(',', ".");
    let bare = bare.as_str();
    for fmt in [
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y/%m/%d %H:%M:%S%.f",
        "%Y/%m/%dT%H:%M:%S%.f",
        // nginx / Apache: 10/Oct/2026:13:55:36
        "%d/%b/%Y:%H:%M:%S",
    ] {
        if let Ok(ndt) = NaiveDateTime::parse_from_str(bare, fmt) {
            return Some(clock_seconds(ndt));
        }
    }
    if let Ok(ndt) = NaiveDateTime::parse_from_str(&format!("{PLACEHOLDER_YEAR}-{t}"), "%Y-%m-%d %H:%M:%S%.f") {
        return Some(clock_seconds(ndt));
    }
    if let Ok(ndt) = NaiveDateTime::parse_from_str(&format!("{PLACEHOLDER_YEAR} {t}"), "%Y %b %e %H:%M:%S") {
        return Some(clock_seconds(ndt));
    }
    None
}

/// Deterministic pseudo-timestamp for formats without a parseable timestamp.
fn infer_incrementing_timestamp(index: usize) -> f64 {
    index as f64 * 0.001
}

fn skip_ws(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && b[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

/// Index just after the closing quote of the string starting at `i`.
fn string_end(b: &[u8], i: usize) -> usize {
    let mut j = i + 1;
    while j < b.len() {
        match b[j] {
            b'\\' => j += 2,
            b'"' => return j + 1,
            _ => j += 1,
        }
    }
    b.len()
}

/// Index just after the object or array starting at `i`.
fn container_end(b: &[u8], i: usize) -> usize {
    let mut depth = 0usize;
    let mut j = i;
    while j < b.len() {
        match b[j] {
            b'"' => {
                j = string_end(b, j);
                continue;
            }
            b'{' | b'[' => depth += 1,
            b'}' | b']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return j + 1;
                }
            }
            _ => {}
        }
        j += 1;
    }
    b.len()
}

/// Byte range of each top-level value of a JSON object line. A string value
/// is the text between its quotes, escapes kept as written; every other
/// value is its whole token.
pub fn json_value_spans(line: &str) -> HashMap<String, Range<usize>> {
    let b = line.as_bytes();
    let mut spans = HashMap::new();
    let mut i = skip_ws(b, 0);
    if b.get(i) != Some(&b'{') {
        return spans;
    }
    i += 1;
    loop {
        i = skip_ws(b, i);
        match b.get(i) {
            Some(b',') => {
                i += 1;
                continue;
            }
            Some(b'"') => {}
            _ => return spans,
        }
        let key_end = string_end(b, i);
        let Some(key) = line.get(i..key_end).and_then(|k| serde_json::from_str::<String>(k).ok()) else {
            return spans;
        };
        i = skip_ws(b, key_end);
        if b.get(i) != Some(&b':') {
            return spans;
        }
        i = skip_ws(b, i + 1);
        let Some(&first) = b.get(i) else { return spans };
        let (range, end) = match first {
            b'"' => {
                let end = string_end(b, i);
                (i + 1..end.saturating_sub(1).max(i + 1), end)
            }
            b'{' | b'[' => {
                let end = container_end(b, i);
                (i..end, end)
            }
            _ => {
                let mut end = i;
                while end < b.len() && !matches!(b[end], b',' | b'}') && !b[end].is_ascii_whitespace() {
                    end += 1;
                }
                (i..end, end)
            }
        };
        spans.insert(key, range);
        i = end;
    }
}

fn parse_json_log_event(
    index: usize,
    json: &Value,
    raw_line: &str,
) -> Option<LogEvent> {
    let key_spans = json_value_spans(raw_line);
    let text_of = |key: &str| key_spans.get(key).map(|r| raw_line[r.clone()].to_string());

    let mut timestamp = infer_incrementing_timestamp(index);
    let mut has_time = false;
    let mut time_str = String::new();
    let mut time_key: Option<&str> = None;
    for key in &["timestamp", "time", "@timestamp", "datetime", "date"] {
        if let Some(val) = json.get(*key).and_then(|v| v.as_str()) {
            time_str = text_of(key).unwrap_or_else(|| val.to_string());
            time_key = Some(key);
            if let Some(t) = read_timestamp(val) {
                timestamp = t;
                has_time = true;
            }
            break;
        }
    }

    let mut level_key: Option<&str> = None;
    for key in &["level", "severity", "loglevel", "log_level"] {
        if json.get(*key).and_then(|v| v.as_str()).is_some() {
            level_key = Some(key);
            break;
        }
    }

    let mut tag_key: Option<&str> = None;
    for key in &["service", "tag", "logger", "module", "component", "caller"] {
        if json.get(*key).and_then(|v| v.as_str()).is_some() {
            tag_key = Some(key);
            break;
        }
    }

    let mut msg_key: Option<&str> = None;
    for key in &["message", "msg", "log", "payload", "text"] {
        if json.get(*key).and_then(|v| v.as_str()).is_some() {
            msg_key = Some(key);
            break;
        }
    }
    let payload = msg_key.and_then(text_of).unwrap_or_else(|| raw_line.to_string());

    let mut event = LogEvent::new(index, timestamp, time_str, payload, raw_line.to_string());
    if !has_time {
        event = event.without_time();
    }

    // Insert every raw JSON key as a dynamic field. A `null` value is no
    // value: its key has no span.
    event.spans = SpanList::from_map(&key_spans);
    if let Value::Object(map) = json {
        for (key, value) in map {
            if value.is_null() {
                event.spans.remove(key);
            }
            event.set_field(key, json_to_log_value(value));
        }
    }

    // Canonical fields, in the text the line has for them.
    let canonical = [("timestamp", time_key), ("level", level_key), ("module", tag_key), ("message", msg_key)];
    for (name, key) in canonical {
        if let Some(range) = key.and_then(|k| key_spans.get(k)) {
            event.spans.insert(name, range.clone());
            if name != "timestamp" {
                event.set_field(name, LogValue::Str(raw_line[range.clone()].to_string()));
            }
        }
    }
    if msg_key.is_none() {
        event.spans.insert("message", 0..raw_line.len());
    }
    if !event.time_str.is_empty() {
        event.set_field("time", LogValue::Str(event.time_str.clone()));
    }

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
/// What reading a stored line again needs: its line number, Generation and
/// text. Copied cheaply out of the cache so it can be read in the background.
pub type StoredLine = (usize, u32, gpui::SharedString);

pub fn stored_lines(entries: &[LogEntry]) -> Vec<StoredLine> {
    entries.iter().map(|e| (e.index, e.generation, e.raw_line.clone())).collect()
}

/// Read stored lines again with `format` (`None`: Auto-detect), across the
/// cores. A line without a time continues the nearest line above it in the
/// same Generation that has one.
pub fn reread(lines: &[StoredLine], format: Option<&CompiledFormat>) -> Vec<LogEntry> {
    let events = crate::par::map_rows(lines.len(), |i| parse_line(lines[i].0, &lines[i].2, format));
    let mut parent: Option<usize> = None;
    let mut generation: Option<u32> = None;
    events
        .into_iter()
        .zip(lines)
        .map(|(mut event, (index, line_gen, _))| {
            if generation != Some(*line_gen) {
                generation = Some(*line_gen);
                parent = None;
            }
            if event.has_time {
                parent = Some(*index);
            } else if let Some(p) = parent {
                event = event.into_continuation(p);
            }
            LogEntry::from_event(&event)
        })
        .collect()
}

/// Take what was read again from `fresh` into `entry`, keeping what is not
/// read from the line (number, Generation, stream, Markers, Breaks).
pub fn adopt(entry: &mut LogEntry, fresh: LogEntry) {
    entry.timestamp = fresh.timestamp;
    entry.has_time = fresh.has_time;
    entry.level = fresh.level;
    entry.own = None;
    entry.spans = fresh.spans;
    entry.extra = Default::default();
    entry.parent = fresh.parent;
    entry.request_id = fresh.request_id;
}

/// Read `entries` again with `format`, in place.
pub fn reread_entries(entries: &mut [LogEntry], format: Option<&CompiledFormat>) {
    let fresh = reread(&stored_lines(entries), format);
    for (entry, f) in entries.iter_mut().zip(fresh) {
        adopt(entry, f);
    }
}
