use crate::marker::MarkerType;
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// 1-based sequential line index in the log file
    pub index: usize,
    /// Unix timestamp in seconds (with subsecond precision)
    pub timestamp: f64,
    /// Formatted time string (e.g. 10:32:02.441)
    pub time_str: String,
    /// Time delta from previous displayed log entry in milliseconds
    pub delta_time_ms: Option<f64>,
    /// Tag / Service / Logger module name (e.g. "auth", "api", "database")
    pub tag: String,
    /// Log severity level
    pub level: LogLevel,
    /// Parsed message payload
    pub payload: String,
    /// Raw unparsed line content
    pub raw_line: String,
    /// Custom Marker Type set by user or parser
    pub marker: Option<MarkerType>,
    /// Optional note written by user
    pub note: Option<String>,
    /// Request ID for distributed tracing investigation
    pub request_id: Option<String>,
    /// Trace ID
    pub trace_id: Option<String>,
    /// Key-value fields from structured JSON log
    pub json_fields: Vec<(String, String)>,
    /// Additional lines if this is a multi-line exception or stack trace
    pub sub_lines: Vec<String>,
    /// Whether multi-line content is expanded in editor
    pub is_expanded: bool,
    /// Highlight marker color if matched by a Marker filter (legacy support)
    pub marker_color: Option<String>,
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
        // Auto-assign passive error/warn marker if applicable
        let auto_marker = MarkerType::from_log_level(level);

        Self {
            index,
            timestamp,
            time_str,
            delta_time_ms: None,
            tag,
            level,
            payload,
            raw_line,
            marker: auto_marker,
            note: None,
            request_id: None,
            trace_id: None,
            json_fields: Vec::new(),
            sub_lines: Vec::new(),
            is_expanded: false,
            marker_color: None,
        }
    }

    /// Effective marker: user-assigned marker or passive error/warn marker
    pub fn effective_marker(&self) -> Option<MarkerType> {
        self.marker.or_else(|| MarkerType::from_log_level(self.level))
    }

    pub fn is_bookmarked(&self) -> bool {
        matches!(self.marker, Some(MarkerType::Bookmark))
    }

    pub fn toggle_bookmark(&mut self) {
        if self.is_bookmarked() {
            self.marker = MarkerType::from_log_level(self.level);
        } else {
            self.marker = Some(MarkerType::Bookmark);
        }
    }
}

impl From<&crate::parser::event::LogEvent> for LogEntry {
    fn from(event: &crate::parser::event::LogEvent) -> Self {
        Self::from_event(event)
    }
}

impl LogEntry {
    /// Convert a schema-typed `LogEvent` back into the rendering model.
    pub fn from_event(event: &crate::parser::event::LogEvent) -> Self {
        let level = event
            .get_field("level")
            .and_then(|v| v.as_str())
            .map(LogLevel::from_str)
            .unwrap_or(LogLevel::Unknown);
        let tag = event
            .get_field("module")
            .and_then(|v| v.as_str())
            .unwrap_or("app")
            .to_string();
        let request_id = event.get_field("requestId").and_then(|v| v.as_str()).map(|s| s.to_string());

        let mut json_fields: Vec<(String, String)> = Vec::new();
        for (name, value) in &event.fields {
            if matches!(name.as_str(), "level" | "module" | "time" | "message" | "requestId") {
                continue;
            }
            json_fields.push((name.clone(), value.to_string_value()));
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
        entry.request_id = request_id;
        entry.json_fields = json_fields;
        entry
    }
}
