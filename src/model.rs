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
pub fn intern(name: &str) -> &'static str {
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

/// Span names that are not fields of their own: the fixed cells, and the
/// JSON keys they were read from under another name.
const NOT_FIELDS: [&str; 6] = ["timestamp", "level", "module", "message", "time", "requestId"];

/// Whether `name` is a field a Field Column can show, not a fixed cell.
pub fn is_field_name(name: &str) -> bool {
    !NOT_FIELDS.contains(&name)
}

/// The name of a field read from lines: a JSON key, a field of a Log Format
/// or an Extraction Rule, a Detected Field. Kept once for the whole run, so
/// it is `Copy` and cheap to compare; it orders by its text.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FieldKey(&'static str);

impl FieldKey {
    pub fn new(name: &str) -> Self {
        Self(intern(name))
    }

    pub fn as_str(self) -> &'static str {
        self.0
    }
}

impl std::fmt::Debug for FieldKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl std::fmt::Display for FieldKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl Serialize for FieldKey {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.0)
    }
}

impl<'de> Deserialize<'de> for FieldKey {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        Ok(FieldKey::new(&name))
    }
}

impl Spans {
    /// Spans by name, in the order of their names.
    pub fn from_map(map: &HashMap<String, Range<usize>>) -> Self {
        let mut pairs: Vec<(&'static str, Range<usize>)> =
            map.iter().map(|(name, range)| (intern(name), range.clone())).collect();
        pairs.sort_by(|a, b| a.0.cmp(b.0));
        Self(pairs.into())
    }

    /// Spans from `(name, range)` pairs, kept in the given order.
    pub fn from_pairs(pairs: Vec<(&'static str, Range<usize>)>) -> Self {
        Self(pairs.into())
    }

    /// The fields among the spans (not the fixed cells), by name.
    pub fn fields(&self) -> impl Iterator<Item = (FieldKey, &Range<usize>)> {
        self.0
            .iter()
            .filter(|(name, _)| is_field_name(name))
            .map(|(name, range)| (FieldKey(name), range))
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

/// Which output of a Remote Command a line came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Stream {
    Stdout,
    Stderr,
}

impl Stream {
    /// The Filter Value and Stream Column text.
    pub fn as_str(self) -> &'static str {
        match self {
            Stream::Stdout => "stdout",
            Stream::Stderr => "stderr",
        }
    }
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
    /// The output of a Remote Command the line came from (`None` for the
    /// lines of files). A line's own: a Continuation Line never inherits it.
    #[serde(default)]
    pub stream: Option<Stream>,
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
    /// Fields Extraction Rules read from the line. A field `spans` already
    /// has keeps its value there (rule 38). Worked out again when the rules
    /// change.
    #[serde(skip)]
    pub extra: Spans,
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
    /// Generation of the file the line was read in; line numbers start again
    /// from 1 in each Generation.
    #[serde(default)]
    pub generation: u32,
    /// A Break drawn just above this row: the file was not continuous here.
    /// Not part of the line; it has no line number and is never copied.
    #[serde(skip)]
    pub break_before: Option<std::sync::Arc<crate::live::BreakMark>>,
    /// A Break drawn just below this row: the lines of a Remote Copy end here
    /// before the end of the file (the read was cancelled or hit a limit).
    #[serde(skip)]
    pub break_after: Option<std::sync::Arc<crate::live::BreakMark>>,
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
            stream: None,
            time_str: time_str.into(),
            delta_time_ms: None,
            tag: tag.into(),
            level,
            level_text: if level == LogLevel::Unknown { SharedString::default() } else { level.as_str().into() },
            spans: Spans::default(),
            extra: Spans::default(),
            parent: None,
            payload: payload.into(),
            raw_line: raw_line.into(),
            markers: Vec::new(),
            request_id: None,
            trace_id: None,
            generation: 0,
            break_before: None,
            break_after: None,
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
    /// The time as written in the line (empty when it has none).
    pub fn time_str(&self) -> &str {
        &self.time_str
    }

    /// The Tag / Service / Logger name as written in the line.
    pub fn tag(&self) -> &str {
        &self.tag
    }

    /// The level exactly as written in the line.
    pub fn level_text(&self) -> &str {
        &self.level_text
    }

    /// The message of the line.
    pub fn payload(&self) -> &str {
        &self.payload
    }

    pub fn set_time_str(&mut self, text: &str) {
        self.time_str = text.to_string().into();
    }

    pub fn set_tag(&mut self, text: &str) {
        self.tag = text.to_string().into();
    }

    /// Whether this line continues the line above it (a stack trace line).
    pub fn is_continuation(&self) -> bool {
        self.parent.is_some()
    }

    /// Where the field `name` is in the line: read with the line, or else
    /// by an Extraction Rule.
    pub fn span(&self, name: &str) -> Option<&Range<usize>> {
        self.spans.get(name).or_else(|| self.extra.get(name))
    }

    /// The text of the field `name` exactly as stored in the line.
    pub fn span_text(&self, name: &str) -> Option<&str> {
        self.span(name).and_then(|r| self.raw_line.get(r.clone()))
    }

    /// Each field of the line (not the fixed cells) with its text: the ones
    /// read with the line by name, then those of Extraction Rules.
    pub fn fields(&self) -> impl Iterator<Item = (FieldKey, &str)> {
        let rules = self.extra.fields().filter(|(key, _)| self.spans.get(key.as_str()).is_none());
        self.spans.fields().chain(rules).filter_map(|(key, r)| Some((key, self.raw_line.get(r.clone())?)))
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
        entry
    }
}
