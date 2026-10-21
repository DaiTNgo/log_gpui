//! Extraction Rules: a name and a regex with exactly one capture group that
//! reads a Field Column from every line of a Project. What the group captures
//! is always a span of the line, never new text (rule 30). Regex only reads
//! columns; Search and Filter never take one (rule 36).
//!
//! Also the guesses "Make column…" offers from a selected stretch of a line,
//! and the Preview of a rule on real lines.

use crate::model::{FieldKey, LogEntry, Spans, intern};
use crate::text_selection::floor_char_boundary;
use regex::{Regex, RegexBuilder, RegexSet};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::Range;
use std::sync::LazyLock;

/// Most memory a compiled rule may take: a pattern past it is refused rather
/// than slowing every line down.
const SIZE_LIMIT: usize = 1 << 20;

/// Lines sampled to rank guesses and preview a rule.
pub const SAMPLE_LINES: usize = 1000;

/// Field names a rule cannot take: the fixed columns and what they are read
/// from.
const RESERVED: [&str; 10] =
    ["timestamp", "level", "module", "message", "source", "stream", "time", "requestId", "request_id", "trace_id"];

/// Where a rule's regex runs: the whole line, or only the MESSAGE cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RuleScope {
    #[default]
    Line,
    Message,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractionRule {
    pub id: u64,
    /// The field the rule reads.
    pub name: String,
    pub pattern: String,
    #[serde(default)]
    pub scope: RuleScope,
    /// How the rule matches, in words ("after `user=`").
    #[serde(default)]
    pub description: String,
    /// A rule turned off is not read; its column has no value.
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
}

fn enabled_by_default() -> bool {
    true
}

/// The next free rule id.
pub fn next_rule_id(rules: &[ExtractionRule]) -> u64 {
    rules.iter().map(|r| r.id).max().map_or(1, |m| m + 1)
}

/// Why a pattern cannot be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatternError {
    Empty,
    /// Not a regex; the parser's message.
    Invalid(String),
    /// The number of capture groups it has (it needs one).
    Groups(usize),
    TooBig,
}

impl PatternError {
    /// One sentence saying how to fix it.
    pub fn message(&self) -> String {
        match self {
            PatternError::Empty => "Type a pattern with one capture group, like user=(\\S+)".into(),
            PatternError::Groups(0) => "Needs exactly one capture group — wrap the value in ( )".into(),
            PatternError::Groups(_) => "Needs exactly one capture group — make the others (?: … )".into(),
            PatternError::TooBig => "The pattern is too large — make it shorter or less repetitive".into(),
            PatternError::Invalid(why) => {
                let last = why.lines().filter(|l| !l.trim().is_empty()).next_back().unwrap_or(why);
                format!("Not a valid pattern: {}", last.trim().trim_start_matches("error: "))
            }
        }
    }
}

/// Compile a rule's pattern: it must have exactly one capture group.
pub fn compile_pattern(pattern: &str) -> Result<Regex, PatternError> {
    if pattern.trim().is_empty() {
        return Err(PatternError::Empty);
    }
    let regex = RegexBuilder::new(pattern).size_limit(SIZE_LIMIT).build().map_err(|e| match e {
        regex::Error::CompiledTooBig(_) => PatternError::TooBig,
        other => PatternError::Invalid(other.to_string()),
    })?;
    match regex.captures_len() - 1 {
        1 => Ok(regex),
        n => Err(PatternError::Groups(n)),
    }
}

/// Where the rule's regex captures in `line`, `message` being the MESSAGE
/// cell. An empty capture is no value.
pub fn capture(regex: &Regex, scope: RuleScope, line: &str, message: Option<Range<usize>>) -> Option<Range<usize>> {
    let (text, offset) = match scope {
        RuleScope::Line => (line, 0),
        RuleScope::Message => {
            let r = message?;
            (line.get(r.clone())?, r.start)
        }
    };
    let m = regex.captures(text)?.get(1)?;
    (!m.is_empty()).then(|| m.start() + offset..m.end() + offset)
}

struct CompiledRule {
    name: &'static str,
    regex: Regex,
    scope: RuleScope,
}

/// How lines are read beyond Auto-detect: the Extraction Rules turned on, in
/// priority order. The same config reads a whole file and each live line.
#[derive(Default)]
pub struct LineConfig {
    rules: Vec<CompiledRule>,
    /// Detected Fields (`key=value` in the message) turned on.
    kv: Vec<&'static str>,
    /// The rules' regexes, to skip at once those that cannot match a line.
    set: Option<RegexSet>,
}

impl std::fmt::Debug for LineConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.rules.iter().map(|r| r.name)).finish()
    }
}

impl LineConfig {
    /// The rules turned on with a usable pattern, in order.
    pub fn new(rules: &[ExtractionRule]) -> Self {
        Self::with_detected(rules, &[])
    }

    /// The rules, and the `key=value` Detected Fields `kv` turned on: those
    /// are read after the rules (rule 38).
    pub fn with_detected(rules: &[ExtractionRule], kv: &[String]) -> Self {
        let rules: Vec<CompiledRule> = rules
            .iter()
            .filter(|r| r.enabled)
            .filter_map(|r| {
                Some(CompiledRule { name: intern(&r.name), regex: compile_pattern(&r.pattern).ok()?, scope: r.scope })
            })
            .collect();
        let set = RegexSet::new(rules.iter().map(|r| r.regex.as_str())).ok().filter(|_| !rules.is_empty());
        Self { rules, kv: kv.iter().map(|k| intern(k)).collect(), set }
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty() && self.kv.is_empty()
    }

    /// The fields the rules read from `entry`. A field the line already has
    /// (a fixed cell, a JSON key) or an earlier rule found is not read again
    /// (rule 38). A Continuation Line has none: it takes its Parent Line's.
    pub fn extra_spans(&self, entry: &LogEntry) -> Spans {
        self.extra_spans_of(&entry.raw_line, &entry.spans, entry.is_continuation())
    }

    fn extra_spans_of(&self, line: &str, spans: &Spans, continuation: bool) -> Spans {
        if self.is_empty() || continuation {
            return Spans::default();
        }
        let message = spans.get("message");
        let maybe = self.set.as_ref().map(|s| s.matches(line));
        let mut found: Vec<(&'static str, Range<usize>)> = Vec::new();
        for (i, rule) in self.rules.iter().enumerate() {
            if spans.get(rule.name).is_some() || found.iter().any(|(n, _)| *n == rule.name) {
                continue;
            }
            // A rule on the message cannot be ruled out by the whole line.
            if rule.scope == RuleScope::Line && maybe.as_ref().is_some_and(|m| !m.matched(i)) {
                continue;
            }
            if let Some(range) = capture(&rule.regex, rule.scope, line, message.clone()) {
                found.push((rule.name, range));
            }
        }
        if !self.kv.is_empty() {
            if let Some(m) = message.and_then(|m| line.get(m.clone()).map(|t| (m.start, t))) {
                for (key, value) in scan_kv(m.1) {
                    let name = &m.1[key];
                    let Some(&wanted) = self.kv.iter().find(|k| **k == name) else { continue };
                    if spans.get(wanted).is_none() && !found.iter().any(|(n, _)| *n == wanted) {
                        found.push((wanted, value.start + m.0..value.end + m.0));
                    }
                }
            }
        }
        if found.is_empty() { Spans::default() } else { Spans::from_pairs(found) }
    }

    /// Read the rules' fields of lines copied out of the view, across the
    /// cores (off the UI thread).
    pub fn read_snapshot(&self, lines: &[LineSnapshot]) -> Vec<Spans> {
        crate::par::map_rows(lines.len(), |i| {
            let l = &lines[i];
            self.extra_spans_of(&l.raw, &l.spans, l.continuation)
        })
    }

    /// Read the rules' fields of every entry, across the cores.
    pub fn read_all(&self, entries: &[LogEntry]) -> Vec<Spans> {
        crate::par::map_rows(entries.len(), |i| self.extra_spans(&entries[i]))
    }

    /// Read the rules' fields of `entries` in place.
    pub fn apply(&self, entries: &mut [LogEntry]) {
        if self.is_empty() && entries.iter().all(|e| e.extra.is_empty()) {
            return;
        }
        let spans = self.read_all(entries);
        for (entry, extra) in entries.iter_mut().zip(spans) {
            entry.extra = extra;
        }
    }
}

/// What reading the rule fields of a line needs, copied cheaply out of the
/// view so it can be read in the background.
#[derive(Clone)]
pub struct LineSnapshot {
    raw: gpui::SharedString,
    spans: Spans,
    continuation: bool,
}

pub fn snapshot(entries: &[LogEntry]) -> Vec<LineSnapshot> {
    entries
        .iter()
        .map(|e| LineSnapshot { raw: e.raw_line.clone(), spans: e.spans.clone(), continuation: e.is_continuation() })
        .collect()
}

// ─── Names ───────────────────────────────────────────────────────────────────

/// What the Name of a new column means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameCheck {
    Empty,
    /// A fixed column's name, which a rule cannot take.
    Reserved(String),
    /// Another Field Column already has the name: values go into it.
    Joins(String),
    New,
}

impl NameCheck {
    pub fn is_ok(&self) -> bool {
        matches!(self, NameCheck::Joins(_) | NameCheck::New)
    }

    /// What to tell the user about the name, if anything.
    pub fn message(&self) -> Option<String> {
        match self {
            NameCheck::Empty => Some("Give the column a name".into()),
            NameCheck::Reserved(n) => Some(format!("\"{n}\" is a built-in column — pick another name")),
            NameCheck::Joins(n) => Some(format!("Values will join column {n}")),
            NameCheck::New => None,
        }
    }
}

/// Check a Name against the fixed columns and the Field Columns `existing`.
pub fn check_name(name: &str, existing: &[&str]) -> NameCheck {
    let name = name.trim();
    if name.is_empty() {
        NameCheck::Empty
    } else if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(name)) {
        NameCheck::Reserved(name.to_string())
    } else if existing.contains(&name) {
        NameCheck::Joins(name.to_string())
    } else {
        NameCheck::New
    }
}

static RE_IP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d{1,3}(?:\.\d{1,3}){3}(?::\d+)?$").unwrap());
static RE_UUID: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$").unwrap()
});
static RE_DURATION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d+(?:\.\d+)?(?:ns|us|µs|ms|s|m|h)$").unwrap());

/// A Name for the column: the key before the selection, else one from the
/// shape of the value (`ip`, `uuid`, `duration`), else none.
pub fn suggest_name(key: Option<&str>, value: &str) -> String {
    if let Some(key) = key {
        return key.to_string();
    }
    let value = value.trim_matches('"');
    if RE_IP.is_match(value) {
        "ip".into()
    } else if RE_UUID.is_match(value) {
        "uuid".into()
    } else if RE_DURATION.is_match(value) {
        "duration".into()
    } else {
        String::new()
    }
}

// ─── Guesses ─────────────────────────────────────────────────────────────────

/// One way to read the selected value from every line.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    /// In words ("after `user=`"); the regex shows only on "Edit regex".
    pub description: String,
    pub pattern: String,
    pub scope: RuleScope,
    /// The key the value follows, for a guess anchored on one.
    pub key: Option<String>,
    /// Sampled lines it reads a value from.
    pub coverage: usize,
}

/// A line of the sample: its text and its MESSAGE cell.
pub type SampleLine<'a> = (&'a str, Option<Range<usize>>);

static RE_KEY_BEFORE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?:^|[^\w.\-"])("?)([A-Za-z_][\w.\-]*)("?)(=|:\s?)("?)$"#).unwrap());

/// Pattern for the value `value`: the narrowest usual class that holds it.
fn value_classes(value: &str, quoted: bool, stop: Option<char>) -> Vec<String> {
    if quoted {
        return vec![r#"((?:[^"\\]|\\.)*)"#.into()];
    }
    let mut classes = Vec::new();
    if !value.is_empty() && value.chars().all(|c| c.is_ascii_digit()) {
        classes.push(r"(\d+)".to_string());
    }
    if value.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
        classes.push(r"([0-9a-fA-F-]+)".to_string());
    }
    if value.chars().all(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '-')) {
        classes.push(r"([\w.\-]+)".to_string());
    }
    if let Some(c) = stop.filter(|c| !c.is_whitespace()) {
        classes.push(format!(r"([^\s{}]+)", regex::escape(&c.to_string())));
    }
    classes.push(r"(\S+)".to_string());
    classes
}

/// The `n`-th stretch between `open` and `close` holding exactly `sel`.
fn bracket_guess(line: &str, sel: &Range<usize>, open: char, close: char, name: &str) -> Option<Candidate> {
    let (o, c) = (regex::escape(&open.to_string()), regex::escape(&close.to_string()));
    let group = Regex::new(&format!(r"{o}([^{c}]*){c}")).ok()?;
    let n = group
        .captures_iter(line)
        .position(|caps| caps.get(1).is_some_and(|m| m.range() == *sel))?;
    let pattern = format!(r"^(?:[^{o}]*{o}[^{c}]*{c}){{{n}}}[^{o}]*{o}([^{c}]*){c}");
    Some(Candidate {
        description: format!("{name} group {}", n + 1),
        pattern,
        scope: RuleScope::Line,
        key: None,
        coverage: 0,
    })
}

/// The `n`-th whitespace-separated word of `text` (starting at `offset` in the
/// line) when it is exactly `sel`.
fn word_guess(text: &str, offset: usize, sel: &Range<usize>, scope: RuleScope, of: &str) -> Option<Candidate> {
    let mut words = Vec::new();
    let mut start = None;
    for (i, ch) in text.char_indices() {
        match (ch.is_whitespace(), start) {
            (false, None) => start = Some(i),
            (true, Some(s)) => {
                words.push(s + offset..i + offset);
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        words.push(s + offset..text.len() + offset);
    }
    let n = words.iter().position(|w| w == sel)?;
    Some(Candidate {
        description: format!("word {} of the {of}", n + 1),
        pattern: format!(r"^\s*(?:\S+\s+){{{n}}}(\S+)"),
        scope,
        key: None,
        coverage: 0,
    })
}

/// The ways to read the stretch `sel` of `line` from every line, best first:
/// only those that read exactly `sel` from `line` are kept, ranked by how
/// many `sample` lines they read a value from. A guess anchored on a key
/// stays ahead of a guess by position that reads as many.
pub fn guess(line: &str, sel: Range<usize>, message: Option<Range<usize>>, sample: &[SampleLine]) -> Vec<Candidate> {
    let Some(value) = line.get(sel.clone()).filter(|v| !v.is_empty()) else { return Vec::new() };
    let mut out: Vec<Candidate> = Vec::new();

    // 1. The key right before the value.
    if let Some(caps) = RE_KEY_BEFORE.captures(&line[..sel.start]) {
        let key = caps.get(2).map_or("", |m| m.as_str()).to_string();
        let prefix = &line[caps.get(1).unwrap().start()..sel.start];
        let quoted = caps.get(5).is_some_and(|m| !m.is_empty());
        let stop = line[sel.end..].chars().next();
        for class in value_classes(value, quoted, stop) {
            let pattern = format!(r"(?:^|[^\w.\-]){}{class}", regex::escape(prefix));
            out.push(Candidate {
                description: format!("after `{}`", prefix.trim_end()),
                pattern,
                scope: RuleScope::Line,
                key: Some(key.clone()),
                coverage: 0,
            });
        }
    }
    // 2. Between brackets.
    out.extend(bracket_guess(line, &sel, '[', ']', "[ ]"));
    out.extend(bracket_guess(line, &sel, '(', ')', "( )"));
    // 3. The n-th word of the message, then of the line.
    if let Some(m) = message.clone().filter(|m| m.start <= sel.start && sel.end <= m.end) {
        out.extend(word_guess(&line[m.clone()], m.start, &sel, RuleScope::Message, "message"));
    }
    out.extend(word_guess(line, 0, &sel, RuleScope::Line, "line"));

    // Keep the guesses that read exactly the selection; one per description.
    let mut kept: Vec<Candidate> = Vec::new();
    for mut c in out {
        let Ok(regex) = compile_pattern(&c.pattern) else { continue };
        if capture(&regex, c.scope, line, message.clone()) != Some(sel.clone()) {
            continue;
        }
        if kept.iter().any(|k| k.description == c.description) {
            continue;
        }
        c.coverage = sample.iter().filter(|(l, m)| capture(&regex, c.scope, l, m.clone()).is_some()).count();
        kept.push(c);
    }
    kept.sort_by(|a, b| b.coverage.cmp(&a.coverage));
    kept
}

/// Up to `max` rows spread evenly over `0..len`.
pub fn sample_rows(len: usize, max: usize) -> Vec<usize> {
    if len <= max {
        return (0..len).collect();
    }
    (0..max).map(|i| i * len / max).collect()
}

// ─── Preview ─────────────────────────────────────────────────────────────────

/// Characters of context kept on each side of the value in a Preview line.
const CONTEXT_CHARS: usize = 36;

/// A line of the Preview, cut around the value read from it.
#[derive(Debug, Clone, PartialEq)]
pub struct PreviewLine {
    /// Row of the view.
    pub row: usize,
    /// The shown part of the line, with `…` where it was cut.
    pub text: String,
    /// The value in `text`; empty for a line not matched.
    pub highlight: Range<usize>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Preview {
    /// Up to 3 lines read, the line of the selection first.
    pub matched: Vec<PreviewLine>,
    /// A line like the first one that the rule does not read.
    pub unmatched: Option<PreviewLine>,
    /// Lines read and lines looked at.
    pub count: usize,
    pub total: usize,
    /// Whether `count` and `total` are of a sample, not every line.
    pub sampled: bool,
    /// Up to 5 most frequent values, with how many lines have each.
    pub top_values: Vec<(String, usize)>,
}

impl Preview {
    /// "1,204 of 1,530 lines (78%)".
    pub fn summary(&self) -> String {
        let pct = if self.total == 0 { 0 } else { self.count * 100 / self.total };
        let lines = format!(
            "{} of {} lines ({pct}%)",
            crate::app::format_count(self.count),
            crate::app::format_count(self.total)
        );
        if self.sampled { format!("{lines} in a sample") } else { lines }
    }
}

/// `line` cut to a window around `value`, with `…` at a cut end; returns the
/// text and where the value is in it.
pub fn cut_around(line: &str, value: Range<usize>) -> (String, Range<usize>) {
    let before = line[..value.start].char_indices().rev().nth(CONTEXT_CHARS - 1).map_or(0, |(i, _)| i);
    let after = line[value.end..].char_indices().nth(CONTEXT_CHARS).map_or(line.len(), |(i, _)| value.end + i);
    let (start, end) = (floor_char_boundary(line, before), floor_char_boundary(line, after));
    let mut text = String::new();
    if start > 0 {
        text.push('…');
    }
    let shift = text.len();
    text.push_str(&line[start..end]);
    if end < line.len() {
        text.push('…');
    }
    (text, value.start - start + shift..value.end - start + shift)
}

/// Lines looked at for the counts of a Preview before it samples instead.
const PREVIEW_EXACT: usize = 20_000;

/// The Preview of `regex` over the rows `entries`, the row `origin` (where the
/// user selected) first. The line not matched is one from the same Source
/// File that holds `key` if the rule is anchored on one.
pub fn preview(regex: &Regex, scope: RuleScope, entries: &[LogEntry], origin: usize, key: Option<&str>) -> Preview {
    let parents: Vec<usize> = (0..entries.len()).filter(|&i| !entries[i].is_continuation()).collect();
    let sampled = parents.len() > PREVIEW_EXACT;
    let rows: Vec<usize> = if sampled {
        sample_rows(parents.len(), PREVIEW_EXACT).into_iter().map(|i| parents[i]).collect()
    } else {
        parents
    };
    let read = |row: usize| {
        let e = &entries[row];
        capture(regex, scope, &e.raw_line, e.spans.get("message"))
    };
    let mut out = Preview { total: rows.len(), sampled, ..Preview::default() };
    let mut values: HashMap<&str, usize> = HashMap::new();
    let origin_entry = entries.get(origin);
    let mut order: Vec<usize> = Vec::with_capacity(rows.len() + 1);
    if origin_entry.is_some() {
        order.push(origin);
    }
    order.extend(rows.iter().copied().filter(|r| *r != origin));
    for &row in &rows {
        if let Some(r) = read(row) {
            out.count += 1;
            *values.entry(&entries[row].raw_line[r]).or_default() += 1;
        }
    }
    let same_kind = |row: usize| {
        let (e, o) = (&entries[row], origin_entry);
        o.is_none_or(|o| o.file_id == e.file_id) && key.is_none_or(|k| e.raw_line.contains(k))
    };
    for row in order {
        match read(row) {
            Some(r) if out.matched.len() < 3 => {
                let (text, highlight) = cut_around(&entries[row].raw_line, r);
                out.matched.push(PreviewLine { row, text, highlight });
            }
            None if out.unmatched.is_none() && same_kind(row) => {
                let line = entries[row].raw_line.as_str();
                let end = line.char_indices().nth(CONTEXT_CHARS * 2).map_or(line.len(), |(i, _)| i);
                let text = if end < line.len() { format!("{}…", &line[..end]) } else { line.to_string() };
                out.unmatched = Some(PreviewLine { row, text, highlight: 0..0 });
            }
            _ => {}
        }
        if out.matched.len() >= 3 && out.unmatched.is_some() {
            break;
        }
    }
    let mut top: Vec<(String, usize)> = values.into_iter().map(|(v, n)| (v.to_string(), n)).collect();
    top.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    top.truncate(5);
    out.top_values = top;
    out
}

/// The Field Columns `rules` read, by name, in rule order (names may repeat
/// across rules).
pub fn rule_fields(rules: &[ExtractionRule]) -> Vec<FieldKey> {
    let mut out: Vec<FieldKey> = Vec::new();
    for r in rules {
        let k = FieldKey::new(&r.name);
        if !out.contains(&k) {
            out.push(k);
        }
    }
    out
}

// ─── Detected Fields: key=value ──────────────────────────────────────────────

/// Whether a key may start right after `prev` (the start of the text, a
/// space or a separator), so `a == b` or `x+y=1` find no key.
fn key_may_start(prev: Option<u8>) -> bool {
    prev.is_none_or(|p| p.is_ascii_whitespace() || matches!(p, b',' | b';' | b'(' | b'[' | b'{'))
}

/// The `key=value` pairs of `text` in logfmt style, as (key, value) byte
/// ranges, the first of each key only. A key is a letter or `_` followed by
/// letters, digits, `_`, `.` or `-`; a value is a quoted string (escapes
/// kept, quotes included) or a run of characters that are not whitespace.
pub fn scan_kv(text: &str) -> Vec<(Range<usize>, Range<usize>)> {
    let b = text.as_bytes();
    let mut out: Vec<(Range<usize>, Range<usize>)> = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let starts_key = (b[i].is_ascii_alphabetic() || b[i] == b'_') && key_may_start(i.checked_sub(1).map(|p| b[p]));
        if !starts_key {
            i += 1;
            continue;
        }
        let mut k = i + 1;
        while k < b.len() && (b[k].is_ascii_alphanumeric() || matches!(b[k], b'_' | b'.' | b'-')) {
            k += 1;
        }
        if b.get(k) != Some(&b'=') || matches!(b.get(k + 1), None | Some(b'=')) || b[k + 1].is_ascii_whitespace() {
            i = k.max(i + 1);
            continue;
        }
        let v = k + 1;
        let end = if b[v] == b'"' {
            let mut j = v + 1;
            while j < b.len() && b[j] != b'"' {
                j += if b[j] == b'\\' { 2 } else { 1 };
            }
            (j + 1).min(b.len())
        } else {
            let mut j = v;
            while j < b.len() && !b[j].is_ascii_whitespace() {
                j += 1;
            }
            j
        };
        let key = i..k;
        if !out.iter().any(|(ok, _)| text[ok.clone()] == text[key.clone()]) {
            out.push((key, v..end));
        }
        i = end;
    }
    out
}

/// The `key=value` keys of an entry's message, each once.
pub fn kv_keys(entry: &LogEntry) -> Vec<FieldKey> {
    let Some(m) = entry.spans.get("message") else { return Vec::new() };
    let Some(text) = entry.raw_line.get(m.clone()) else { return Vec::new() };
    scan_kv(text).into_iter().map(|(k, _)| FieldKey::new(&text[k])).collect()
}
