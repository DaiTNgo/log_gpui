use crate::model::LogLevel;
use std::collections::HashMap;

/// A typed value extracted from a structured log field.
#[derive(Debug, Clone, PartialEq)]
pub enum LogValue {
    Str(String),
    Number(f64),
    Bool(bool),
    Null,
}

impl LogValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            LogValue::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            LogValue::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            LogValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// String representation used for contains/startsWith/endsWith matching.
    pub fn to_string_value(&self) -> String {
        match self {
            LogValue::Str(s) => s.clone(),
            LogValue::Number(n) => n.to_string(),
            LogValue::Bool(b) => b.to_string(),
            LogValue::Null => String::new(),
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, LogValue::Null)
    }
}

/// A schema-typed log line. `fields` holds dynamically-discovered values drawn
/// from the per-file `LogSchema` — never hard-coded onto the event.
#[derive(Debug, Clone, PartialEq)]
pub struct LogEvent {
    /// 1-based sequential line index in the log file
    pub index: usize,
    /// Unix timestamp in seconds (with subsecond precision)
    pub timestamp: f64,
    /// Formatted time string (e.g. 10:32:02.441)
    pub time_str: String,
    /// Dynamic structured fields keyed by schema field name
    pub fields: HashMap<String, LogValue>,
    /// Human-readable primary payload line
    pub message: String,
    /// Raw unparsed line content
    pub raw_line: String,
    /// Whether `timestamp` was read from the line. Lines without a time of
    /// their own get a pseudo timestamp from their line number.
    pub has_time: bool,
}

impl LogEvent {
    pub fn new(
        index: usize,
        timestamp: f64,
        time_str: String,
        message: String,
        raw_line: String,
    ) -> Self {
        Self {
            index,
            timestamp,
            time_str,
            fields: HashMap::new(),
            message,
            raw_line,
            has_time: timestamp.is_finite() && timestamp > 0.0,
        }
    }

    /// Mark the timestamp as a pseudo value, not read from the line.
    pub fn without_time(mut self) -> Self {
        self.has_time = false;
        self
    }

    /// Fill the field map with the canonical log metadata (level, module, ...)
    /// from the parsed log line.
    pub fn with_metadata(mut self, level: LogLevel, tag: &str) -> Self {
        self.fields.insert(
            "level".to_string(),
            LogValue::Str(level.as_str().to_string()),
        );
        self.fields.insert(
            "module".to_string(),
            LogValue::Str(tag.to_string()),
        );
        self.fields
            .insert("time".to_string(), LogValue::Str(self.time_str.clone()));
        self
    }

    /// Insert a structured field value, overwriting any existing entry.
    pub fn set_field(&mut self, name: &str, value: LogValue) {
        self.fields.insert(name.to_string(), value);
    }

    pub fn get_field(&self, name: &str) -> Option<&LogValue> {
        self.fields.get(name)
    }
}

/// Convert a `serde_json::Value` into a `LogValue`, recursing into simple
/// containers only; nested objects/arrays collapse to a JSON string.
pub fn json_to_log_value(value: &serde_json::Value) -> LogValue {
    match value {
        serde_json::Value::Null => LogValue::Null,
        serde_json::Value::Bool(b) => LogValue::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                LogValue::Number(f)
            } else {
                LogValue::Str(n.to_string())
            }
        }
        serde_json::Value::String(s) => LogValue::Str(s.clone()),
        other => LogValue::Str(other.to_string()),
    }
}