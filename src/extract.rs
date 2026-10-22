//! Detected Fields: the `key=value` pairs of a line's message, read as Field
//! Columns while their switch in the Columns section is on. What a field
//! reads is always a span of the line, never new text.

use crate::model::{FieldKey, LogEntry, Spans, intern};
use std::ops::Range;

/// How lines are read beyond Auto-detect: the `key=value` Detected Fields
/// turned on. The same config reads a whole file and each live line.
#[derive(Default)]
pub struct LineConfig {
    /// Detected Fields (`key=value` in the message) turned on.
    kv: Vec<&'static str>,
}

impl std::fmt::Debug for LineConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.kv.iter()).finish()
    }
}

impl LineConfig {
    /// Read the `key=value` Detected Fields `kv`.
    pub fn new(kv: &[String]) -> Self {
        Self { kv: kv.iter().map(|k| intern(k)).collect() }
    }

    pub fn is_empty(&self) -> bool {
        self.kv.is_empty()
    }

    /// The fields read from `entry`. A field the line already has (a fixed
    /// cell, a JSON key, a Log Format's) is not read again. A Continuation
    /// Line has none: it takes its Parent Line's.
    pub fn extra_spans(&self, entry: &LogEntry) -> Spans {
        self.extra_spans_of(&entry.raw_line, &entry.spans, entry.is_continuation())
    }

    fn extra_spans_of(&self, line: &str, spans: &Spans, continuation: bool) -> Spans {
        if self.is_empty() || continuation {
            return Spans::default();
        }
        let Some(message) = spans.get("message") else { return Spans::default() };
        let Some(text) = line.get(message.clone()) else { return Spans::default() };
        let mut found: Vec<(&'static str, Range<usize>)> = Vec::new();
        for (key, value) in scan_kv(text) {
            let name = &text[key];
            let Some(&wanted) = self.kv.iter().find(|k| **k == name) else { continue };
            if spans.get(wanted).is_none() && !found.iter().any(|(n, _)| *n == wanted) {
                found.push((wanted, value.start + message.start..value.end + message.start));
            }
        }
        if found.is_empty() { Spans::default() } else { Spans::from_pairs(found) }
    }

    /// Read the fields of lines copied out of the view, across the cores (off
    /// the UI thread).
    pub fn read_snapshot(&self, lines: &[LineSnapshot]) -> Vec<Spans> {
        crate::par::map_rows(lines.len(), |i| {
            let l = &lines[i];
            self.extra_spans_of(&l.raw, &l.spans, l.continuation)
        })
    }

    /// Read the fields of every entry, across the cores.
    pub fn read_all(&self, entries: &[LogEntry]) -> Vec<Spans> {
        crate::par::map_rows(entries.len(), |i| self.extra_spans(&entries[i]))
    }

    /// Read the fields of `entries` in place.
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

/// What reading the fields of a line needs, copied cheaply out of the view so
/// it can be read in the background.
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
