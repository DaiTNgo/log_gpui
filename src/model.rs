use crate::marker::TextMarker;
use gpui::SharedString;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum LogLevel {
    Verbose,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
    Unknown,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Verbose => "VERBOSE",
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
            LogLevel::Fatal => "FATAL",
            LogLevel::Unknown => "UNKNOWN",
        }
    }

    pub fn short_code(&self) -> &'static str {
        match self {
            LogLevel::Verbose => "V",
            LogLevel::Debug => "D",
            LogLevel::Info => "I",
            LogLevel::Warn => "W",
            LogLevel::Error => "E",
            LogLevel::Fatal => "F",
            LogLevel::Unknown => "?",
        }
    }

    /// The level a word stands for when the whole word is a level name or
    /// abbreviation (`E`, `err`, `WARNING`), else `Unknown`.
    pub fn from_token(token: &str) -> Self {
        match token.trim().to_ascii_lowercase().as_str() {
            "f" | "fatal" | "crit" | "critical" => LogLevel::Fatal,
            "e" | "err" | "error" => LogLevel::Error,
            "w" | "wrn" | "warn" | "warning" => LogLevel::Warn,
            "i" | "inf" | "info" => LogLevel::Info,
            "d" | "dbg" | "debug" => LogLevel::Debug,
            "v" | "verb" | "verbose" | "trace" => LogLevel::Verbose,
            _ => LogLevel::Unknown,
        }
    }

    pub fn from_str(s: &str) -> Self {
        let upper = s.to_uppercase();
        if upper.contains("FATAL") || upper.contains("CRIT") {
            LogLevel::Fatal
        } else if upper.contains("ERROR") || upper.contains("ERR") {
            LogLevel::Error
        } else if upper.contains("WARN") || upper.contains("WRN") {
            LogLevel::Warn
        } else if upper.contains("INFO") || upper.contains("INF") {
            LogLevel::Info
        } else if upper.contains("DEBUG") || upper.contains("DBG") {
            LogLevel::Debug
        } else if upper.contains("VERB") || upper.contains("TRACE") {
            LogLevel::Verbose
        } else {
            LogLevel::Unknown
        }
    }
}

/// The byte range in `raw_line` of each field read from the line, by field
/// name. A line has a handful of them, so a plain list takes far less memory
/// than a map; the names are shared, never copied per line, and so is the
/// list when an entry is copied into a view.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "HashMap<String, Range<usize>>", into = "HashMap<String, Range<usize>>")]
pub struct Spans(std::sync::Arc<[(&'static str, Range<usize>)]>);

/// The one copy of a field name, kept for the rest of the run: names come
/// from the fields of the logs read, so there are few of them.
fn intern(name: &str) -> &'static str {
    use std::cell::RefCell;
    use std::collections::HashSet;
    thread_local! {
        static NAMES: RefCell<HashSet<&'static str>> = RefCell::new(HashSet::new());
    }
    NAMES.with(|names| {
        let mut names = names.borrow_mut();
        if let Some(known) = names.get(name) {
            return *known;
        }
        let leaked: &'static str = Box::leak(name.to_string().into_boxed_str());
        names.insert(leaked);
        leaked
    })
}

impl Spans {
    pub fn from_map(map: &HashMap<String, Range<usize>>) -> Self {
        Self(map.iter().map(|(name, range)| (intern(name), range.clone())).collect())
    }

    pub fn get(&self, name: &str) -> Option<&Range<usize>> {
        self.0.iter().find(|(key, _)| *key == name).map(|(_, range)| range)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&'static str, &Range<usize>)> {
        self.0.iter().map(|(name, range)| (*name, range))
    }
}

impl From<HashMap<String, Range<usize>>> for Spans {
    fn from(map: HashMap<String, Range<usize>>) -> Self {
        Self::from_map(&map)
    }
}

impl From<Spans> for HashMap<String, Range<usize>> {
    fn from(spans: Spans) -> Self {
        spans.0.iter().map(|(name, range)| (name.to_string(), range.clone())).collect()
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// 1-based sequential line index in the log file
    pub index: usize,
    /// Unix timestamp in seconds (with subsecond precision)
    pub timestamp: f64,
    /// Whether `timestamp` was read from the line (a continuation line or a
    /// line without a time has only a pseudo timestamp).
    #[serde(default = "default_true")]
    pub has_time: bool,
    /// Formatted time string (e.g. 10:32:02.441)
    pub time_str: SharedString,
    /// Time delta from previous displayed log entry in milliseconds
    pub delta_time_ms: Option<f64>,
    /// Tag / Service / Logger module name (e.g. "auth", "api", "database")
    pub tag: SharedString,
    /// Log severity level, as understood from `level_text`
    pub level: LogLevel,
    /// The level exactly as written in the line (`err`, `E`, `ERROR`); empty
    /// when the line has none.
    #[serde(default)]
    pub level_text: SharedString,
    /// Byte range in `raw_line` of each field read from the line.
    #[serde(default)]
    pub spans: Spans,
    /// Line number of the Parent Line when this is a Continuation Line.
    #[serde(default)]
    pub parent: Option<usize>,
    /// Parsed message payload
    pub payload: SharedString,
    /// Raw unparsed line content
    pub raw_line: SharedString,
    /// Colors the user painted on stretches of this line's text.
    #[serde(default)]
    pub markers: Vec<TextMarker>,
    /// Request ID for distributed tracing investigation
    pub request_id: Option<String>,
    /// Trace ID
    pub trace_id: Option<String>,
    /// Key-value fields from structured JSON log
    pub json_fields: Vec<(String, String)>,
    /// Generation of the file the line was read in; line numbers start again
    /// from 1 in each Generation.
    #[serde(default)]
    pub generation: u32,
    /// A Break drawn just above this row: the file was not continuous here.
    /// Not part of the line; it has no line number and is never copied.
    #[serde(skip)]
    pub break_before: Option<std::sync::Arc<crate::live::BreakMark>>,
    /// `file_id` of the Source File when the entry belongs to a Log Space.
    #[serde(skip)]
    pub file_id: Option<std::sync::Arc<str>>,
    /// Display name of the Source File, shown in the Source Column. Only set
    /// on the rows of a Log Space's merged timeline.
    #[serde(skip)]
    pub source: Option<std::sync::Arc<str>>,
}

impl LogEntry {
    pub fn new(
        index: usize,
        timestamp: f64,
        time_str: String,
        tag: String,
        level: LogLevel,
        payload: String,
        raw_line: String,
    ) -> Self {
        Self {
            index,
            timestamp,
            has_time: timestamp.is_finite() && timestamp > 0.0,
            time_str: time_str.into(),
            delta_time_ms: None,
            tag: tag.into(),
            level,
            level_text: if level == LogLevel::Unknown { SharedString::default() } else { level.as_str().into() },
            spans: Spans::default(),
            parent: None,
            payload: payload.into(),
            raw_line: raw_line.into(),
            markers: Vec::new(),
            request_id: None,
            trace_id: None,
            json_fields: Vec::new(),
            generation: 0,
            break_before: None,
            file_id: None,
            source: None,
        }
    }
}

impl From<&crate::parser::event::LogEvent> for LogEntry {
    fn from(event: &crate::parser::event::LogEvent) -> Self {
        Self::from_event(event)
    }
}

impl LogEntry {
    /// Whether this line continues the line above it (a stack trace line).
    pub fn is_continuation(&self) -> bool {
        self.parent.is_some()
    }

    /// The text of the field `name` exactly as stored in the line.
    pub fn span_text(&self, name: &str) -> Option<&str> {
        self.spans.get(name).and_then(|r| self.raw_line.get(r.clone()))
    }

    /// Convert a schema-typed `LogEvent` back into the rendering model. Every
    /// text the entry shows is read from the line itself.
    pub fn from_event(event: &crate::parser::event::LogEvent) -> Self {
        let level_text = event
            .span_text("level")
            .map(str::to_string)
            .or_else(|| event.get_field("level").and_then(|v| v.as_str()).map(str::to_string))
            .unwrap_or_default();
        let level = match LogLevel::from_token(&level_text) {
            LogLevel::Unknown if !level_text.is_empty() => LogLevel::from_str(&level_text),
            known => known,
        };
        let tag = event
            .span_text("module")
            .map(str::to_string)
            .or_else(|| event.get_field("module").and_then(|v| v.as_str()).map(str::to_string))
            .unwrap_or_default();
        let request_id = event.get_field("requestId").and_then(|v| v.as_str()).map(|s| s.to_string());

        let mut json_fields: Vec<(String, String)> = Vec::new();
        for (name, value) in &event.fields {
            if matches!(name.as_str(), "level" | "module" | "time" | "message" | "requestId") {
                continue;
            }
            let text = match value {
                crate::parser::event::LogValue::Null => String::new(),
                other => event
                    .span_text(name)
                    .map(str::to_string)
                    .unwrap_or_else(|| other.to_string_value()),
            };
            json_fields.push((name.clone(), text));
        }
        json_fields.sort();

        let mut entry = LogEntry::new(
            event.index,
            event.timestamp,
            event.time_str.clone(),
            tag,
            level,
            event.message.clone(),
            event.raw_line.clone(),
        );
        entry.level_text = level_text.into();
        entry.spans = Spans::from_map(&event.spans);
        entry.parent = event.parent;
        entry.has_time = event.has_time;
        entry.request_id = request_id;
        entry.json_fields = json_fields;
        entry
    }
}
