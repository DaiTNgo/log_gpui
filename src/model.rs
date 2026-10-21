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
/// than a map: each is 12 bytes (an id of the interned name and two `u32`
/// offsets), the names are kept once for the run, and the list is shared when
/// an entry is copied into a view. No spans (the usual case for `extra`) is
/// no allocation at all.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "HashMap<String, Range<usize>>", into = "HashMap<String, Range<usize>>")]
pub struct Spans(Option<std::sync::Arc<[PackedSpan]>>);

#[derive(Clone, Copy, PartialEq, Eq)]
struct PackedSpan {
    name: u32,
    start: u32,
    end: u32,
}

impl PackedSpan {
    fn new(name: &str, range: &Range<usize>) -> Self {
        let off = |v: usize| u32::try_from(v).unwrap_or(u32::MAX);
        Self { name: name_id(name), start: off(range.start), end: off(range.end) }
    }

    fn range(self) -> Range<usize> {
        self.start as usize..self.end as usize
    }
}

struct NameTable {
    ids: HashMap<&'static str, u32>,
    names: Vec<&'static str>,
}

fn name_table() -> &'static std::sync::Mutex<NameTable> {
    static TABLE: std::sync::OnceLock<std::sync::Mutex<NameTable>> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| std::sync::Mutex::new(NameTable { ids: HashMap::new(), names: Vec::new() }))
}

thread_local! {
    /// This thread's copy of the names known so far, so reading a name or an
    /// id takes no lock.
    static KNOWN: std::cell::RefCell<NameTable> =
        std::cell::RefCell::new(NameTable { ids: HashMap::new(), names: Vec::new() });
}

fn remember(id: u32, name: &'static str) {
    KNOWN.with(|k| {
        let mut k = k.borrow_mut();
        k.ids.insert(name, id);
        if k.names.len() <= id as usize {
            k.names.resize(id as usize + 1, "");
        }
        k.names[id as usize] = name;
    });
}

/// The id of `name`, adding it to the table when `add` is set.
fn find_id(name: &str, add: bool) -> Option<u32> {
    if let Some(id) = KNOWN.with(|k| k.borrow().ids.get(name).copied()) {
        return Some(id);
    }
    let mut table = name_table().lock().unwrap_or_else(|e| e.into_inner());
    let id = match table.ids.get(name) {
        Some(&id) => id,
        None if add => {
            let leaked: &'static str = Box::leak(name.to_string().into_boxed_str());
            let id = table.names.len() as u32;
            table.names.push(leaked);
            table.ids.insert(leaked, id);
            id
        }
        None => return None,
    };
    let known: &'static str = table.names[id as usize];
    drop(table);
    remember(id, known);
    Some(id)
}

fn name_id(name: &str) -> u32 {
    find_id(name, true).expect("added")
}

/// The name behind an id made by `name_id`.
fn name_of(id: u32) -> &'static str {
    let cached = KNOWN.with(|k| k.borrow().names.get(id as usize).copied().filter(|n| !n.is_empty()));
    if let Some(name) = cached {
        return name;
    }
    let known = name_table().lock().unwrap_or_else(|e| e.into_inner()).names[id as usize];
    remember(id, known);
    known
}

/// The one copy of a field name, kept for the rest of the run: names come
/// from the fields of the logs read, so there are few of them.
pub fn intern(name: &str) -> &'static str {
    name_of(name_id(name))
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
        let mut pairs: Vec<(&str, &Range<usize>)> = map.iter().map(|(name, range)| (name.as_str(), range)).collect();
        pairs.sort_by(|a, b| a.0.cmp(b.0));
        Self::pack(pairs.into_iter().map(|(name, range)| PackedSpan::new(name, range)))
    }

    /// Spans from `(name, range)` pairs, kept in the given order.
    pub fn from_pairs(pairs: Vec<(&'static str, Range<usize>)>) -> Self {
        Self::pack(pairs.iter().map(|(name, range)| PackedSpan::new(name, range)))
    }

    fn pack(spans: impl Iterator<Item = PackedSpan>) -> Self {
        let list: Vec<PackedSpan> = spans.collect();
        if list.is_empty() { Self(None) } else { Self(Some(list.into())) }
    }

    fn packed(&self) -> &[PackedSpan] {
        self.0.as_deref().unwrap_or(&[])
    }

    /// The fields among the spans (not the fixed cells), by name.
    pub fn fields(&self) -> impl Iterator<Item = (FieldKey, Range<usize>)> + '_ {
        self.iter().filter(|(name, _)| is_field_name(name)).map(|(name, range)| (FieldKey(name), range))
    }

    pub fn get(&self, name: &str) -> Option<Range<usize>> {
        let id = find_id(name, false)?;
        self.packed().iter().find(|s| s.name == id).map(|s| s.range())
    }

    pub fn len(&self) -> usize {
        self.packed().len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_none()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&'static str, Range<usize>)> + '_ {
        self.packed().iter().map(|s| (name_of(s.name), s.range()))
    }

    /// Bytes this list takes on the heap (not counting a copy that shares it).
    pub fn heap_bytes(&self) -> usize {
        self.0.as_ref().map_or(0, |list| 2 * std::mem::size_of::<usize>() + list.len() * std::mem::size_of::<PackedSpan>())
    }
}

impl std::fmt::Debug for Spans {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl From<HashMap<String, Range<usize>>> for Spans {
    fn from(map: HashMap<String, Range<usize>>) -> Self {
        Self::from_map(&map)
    }
}

impl From<Spans> for HashMap<String, Range<usize>> {
    fn from(spans: Spans) -> Self {
        spans.iter().map(|(name, range)| (name.to_string(), range)).collect()
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

/// The texts of an entry that does not have them in its line.
#[derive(Debug, Clone, Default)]
pub(crate) struct OwnTexts {
    time_str: SharedString,
    tag: SharedString,
    level_text: SharedString,
    payload: SharedString,
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
    /// Time delta from previous displayed log entry in milliseconds
    pub delta_time_ms: Option<f64>,
    /// Log severity level, as understood from the level text of the line
    pub level: LogLevel,
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
    /// Raw unparsed line content: the one copy of the line's text. The time,
    /// level text, tag and payload are read from it by `spans`.
    pub raw_line: SharedString,
    /// Texts that are not stretches of `raw_line`: only entries made by hand
    /// (`LogEntry::new`, a setter) have them; a parsed entry reads its texts
    /// from `raw_line` and has none.
    #[serde(skip)]
    pub(crate) own: Option<std::sync::Arc<OwnTexts>>,
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
            delta_time_ms: None,
            level,
            spans: Spans::default(),
            extra: Spans::default(),
            parent: None,
            raw_line: raw_line.into(),
            own: Some(std::sync::Arc::new(OwnTexts {
                time_str: time_str.into(),
                tag: tag.into(),
                level_text: if level == LogLevel::Unknown { SharedString::default() } else { level.as_str().into() },
                payload: payload.into(),
            })),
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
    /// The text of the field `name` in the line, empty when it has none.
    fn text_of(&self, name: &str) -> &str {
        self.span_text(name).unwrap_or("")
    }

    /// The time as written in the line (empty when it has none).
    pub fn time_str(&self) -> &str {
        match &self.own {
            Some(own) => &own.time_str,
            None => self.text_of("timestamp"),
        }
    }

    /// The Tag / Service / Logger name as written in the line.
    pub fn tag(&self) -> &str {
        match &self.own {
            Some(own) => &own.tag,
            None => self.text_of("module"),
        }
    }

    /// The level exactly as written in the line (`err`, `E`, `ERROR`); empty
    /// when the line has none.
    pub fn level_text(&self) -> &str {
        match &self.own {
            Some(own) => &own.level_text,
            None => self.text_of("level"),
        }
    }

    /// The message of the line.
    pub fn payload(&self) -> &str {
        match &self.own {
            Some(own) => &own.payload,
            None => self.text_of("message"),
        }
    }

    /// The four texts as owned ones, to change one of them.
    fn own_mut(&mut self) -> &mut OwnTexts {
        if self.own.is_none() {
            let own = OwnTexts {
                time_str: self.time_str().to_string().into(),
                tag: self.tag().to_string().into(),
                level_text: self.level_text().to_string().into(),
                payload: self.payload().to_string().into(),
            };
            self.own = Some(std::sync::Arc::new(own));
        }
        std::sync::Arc::make_mut(self.own.as_mut().expect("just set"))
    }

    pub fn set_time_str(&mut self, text: &str) {
        self.own_mut().time_str = text.to_string().into();
    }

    pub fn set_tag(&mut self, text: &str) {
        self.own_mut().tag = text.to_string().into();
    }

    /// Whether this line continues the line above it (a stack trace line).
    pub fn is_continuation(&self) -> bool {
        self.parent.is_some()
    }

    /// Where the field `name` is in the line: read with the line, or else
    /// by an Extraction Rule.
    pub fn span(&self, name: &str) -> Option<Range<usize>> {
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
        Self::from_event_text(event, event.raw_line.as_str().into())
    }

    /// The same, with the line's text already in the shared form.
    pub fn from_event_text(event: &crate::parser::event::LogEvent, raw_line: SharedString) -> Self {
        let field = |name: &str| event.get_field(name).and_then(|v| v.as_str());
        let level_text = event.span_text("level").or_else(|| field("level")).unwrap_or("");
        let level = match LogLevel::from_token(level_text) {
            LogLevel::Unknown if !level_text.is_empty() => LogLevel::from_str(level_text),
            known => known,
        };
        let tag = event.span_text("module").or_else(|| field("module")).unwrap_or("");
        let request_id = event.get_field("requestId").and_then(|v| v.as_str()).map(|s| s.to_string());

        let mut entry = LogEntry::new(event.index, event.timestamp, String::new(), String::new(), level, String::new(), String::new());
        entry.own = None;
        entry.raw_line = raw_line;
        entry.spans = event.spans.to_spans();
        entry.parent = event.parent;
        entry.has_time = event.has_time;
        entry.request_id = request_id;
        // An event not read from its line (made by hand) can have texts the
        // line does not: keep them. A parsed one never does.
        if entry.time_str() != event.time_str
            || entry.payload() != event.message
            || entry.tag() != tag
            || entry.level_text() != level_text
        {
            entry.own = Some(std::sync::Arc::new(OwnTexts {
                time_str: event.time_str.as_str().into(),
                tag: tag.to_string().into(),
                level_text: level_text.to_string().into(),
                payload: event.message.as_str().into(),
            }));
        }
        entry
    }
}
