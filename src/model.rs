use crate::marker::TextMarker;
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
    pub time_str: String,
    /// Time delta from previous displayed log entry in milliseconds
    pub delta_time_ms: Option<f64>,
    /// Tag / Service / Logger module name (e.g. "auth", "api", "database")
    pub tag: String,
    /// Log severity level, as understood from `level_text`
    pub level: LogLevel,
    /// The level exactly as written in the line (`err`, `E`, `ERROR`); empty
    /// when the line has none.
    #[serde(default)]
    pub level_text: String,
    /// Byte range in `raw_line` of each field read from the line.
    #[serde(default)]
    pub spans: HashMap<String, Range<usize>>,
    /// Line number of the Parent Line when this is a Continuation Line.
    #[serde(default)]
    pub parent: Option<usize>,
    /// Parsed message payload
    pub payload: String,
    /// Raw unparsed line content
    pub raw_line: String,
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
            time_str,
            delta_time_ms: None,
            tag,
            level,
            level_text: if level == LogLevel::Unknown { String::new() } else { level.as_str().to_string() },
            spans: HashMap::new(),
            parent: None,
            payload,
            raw_line,
            markers: Vec::new(),
            request_id: None,
            trace_id: None,
            json_fields: Vec::new(),
            generation: 0,
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
        entry.level_text = level_text;
        entry.spans = event.spans.clone();
        entry.parent = event.parent;
        entry.has_time = event.has_time;
        entry.request_id = request_id;
        entry.json_fields = json_fields;
        entry
    }
}
