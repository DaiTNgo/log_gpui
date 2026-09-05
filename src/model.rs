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
    /// Formatted time string
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
    /// Whether this entry is bookmarked by user
    pub is_bookmarked: bool,
    /// Highlight marker color if matched by a Marker filter
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
        Self {
            index,
            timestamp,
            time_str,
            delta_time_ms: None,
            tag,
            level,
            payload,
            raw_line,
            is_bookmarked: false,
            marker_color: None,
        }
    }
}
