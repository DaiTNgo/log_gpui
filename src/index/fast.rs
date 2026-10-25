//! A reader of the most common line layout that allocates nothing beyond
//! the Detected Fields it finds:
//!
//! `2026-09-05 12:34:56.789 [INFO] [auth] message` (or `T` between date and
//! time, `/` in the date, `,` before the fraction, the level without
//! brackets, no tag).
//!
//! It reads a line exactly as Auto-detect does, or gives up (`None`) and
//! leaves the line to Auto-detect: on anything it is not sure of (a zone, a
//! tab, a `:` after the level, a bracket that does not close) it gives up.
//! A file locks on it (decision 7) once its first lines read alike both
//! ways; the spike measured Indexing ~10x faster with it.

use crate::filter_index::LineFacts;
use crate::model::{FieldKey, LogLevel};
use crate::parser::text::{LineConfig, is_access_line};
use std::ops::Range;

fn digits(b: &[u8], r: Range<usize>) -> Option<u32> {
    let mut v = 0u32;
    for &c in b.get(r)? {
        if !c.is_ascii_digit() {
            return None;
        }
        v = v * 10 + u32::from(c - b'0');
    }
    Some(v)
}

/// The time at the start of `b`: its end, and its value as `read_timestamp`
/// reads it.
fn read_time(b: &[u8]) -> Option<(usize, f64)> {
    if b.len() < 19 {
        return None;
    }
    let sep = b[4];
    if !(sep == b'-' || sep == b'/') || b[7] != sep || !(b[10] == b' ' || b[10] == b'T') || b[13] != b':' || b[16] != b':' {
        return None;
    }
    let (y, mo, d) = (digits(b, 0..4)?, digits(b, 5..7)?, digits(b, 8..10)?);
    let (h, mi, s) = (digits(b, 11..13)?, digits(b, 14..16)?, digits(b, 17..19)?);
    let mut end = 19;
    let mut nanos = 0;
    if matches!(b.get(end), Some(b'.' | b',')) {
        let start = end + 1;
        end = start;
        while end < b.len() && b[end].is_ascii_digit() {
            end += 1;
        }
        let n = end - start;
        if n == 0 || n > 9 {
            return None;
        }
        nanos = digits(b, start..end)? * 10u32.pow((9 - n) as u32);
    }
    let utc = chrono::NaiveDate::from_ymd_opt(y as i32, mo, d)?.and_hms_nano_opt(h, mi, s, nanos)?.and_utc();
    let seconds = utc.timestamp() as f64 + f64::from(utc.timestamp_subsec_micros()) / 1_000_000.0;
    Some((end, seconds))
}

/// Spaces from `at`: where they end, when there is at least one and text
/// after them.
fn spaces(b: &[u8], at: usize) -> Option<usize> {
    let mut end = at;
    while end < b.len() && b[end] == b' ' {
        end += 1;
    }
    (end > at && end < b.len()).then_some(end)
}

fn is_tag_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.')
}

/// The facts of `line` read the fast way, or `None` to read it with
/// Auto-detect. `config` gives the Detected Fields to read.
pub fn read_line<'a>(line: &'a str, config: &LineConfig) -> Option<LineFacts<'a>> {
    let text = line.trim_end();
    let b = text.as_bytes();
    let (time_end, seconds) = read_time(b)?;
    // A time before 1970 is no time to the parser.
    if !(seconds.is_finite() && seconds > 0.0) {
        return None;
    }
    let mut at = spaces(b, time_end)?;

    // The level, in brackets or bare.
    let bracket = b[at] == b'[';
    let level_start = at + usize::from(bracket);
    let mut level_end = level_start;
    while level_end < b.len() && b[level_end].is_ascii_alphabetic() {
        level_end += 1;
    }
    if level_end == level_start || level_end - level_start > 7 {
        return None;
    }
    at = level_end;
    if bracket {
        if b.get(at) != Some(&b']') {
            return None;
        }
        at += 1;
    }
    let level_text = &text[level_start..level_end];
    let level = LogLevel::from_token(level_text);
    if level == LogLevel::Unknown {
        return None;
    }
    at = spaces(b, at)?;

    // The tag, when the next word is one in brackets.
    let mut tag_range = None;
    if b[at] == b'[' {
        let start = at + 1;
        let mut end = start;
        while end < b.len() && is_tag_char(b[end]) {
            end += 1;
        }
        if end == start || b.get(end) != Some(&b']') {
            return None;
        }
        tag_range = Some(start..end);
        at = spaces(b, end + 1)?;
    }
    let message = &text[at..];
    // An access log line is read before this layout.
    if message.contains("] \"") && is_access_line(text) {
        return None;
    }

    let pairs = crate::extract::scan_kv(message);
    let has_tag = tag_range.is_some();
    let taken = |name: &str| matches!(name, "timestamp" | "level" | "message") || (has_tag && name == "module");
    let found = config.wanted_kv(message, &pairs, taken);
    let tag = match &tag_range {
        Some(r) => &text[r.clone()],
        None => found.iter().find(|(n, _)| *n == "module").map_or("", |(_, r)| &message[r.clone()]),
    };
    Some(LineFacts {
        has_time: true,
        timestamp: Some(seconds),
        level,
        time_str: &text[..time_end],
        level_text,
        tag,
        request_id: "",
        trace_id: "",
        source: None,
        stream: None,
        fields: Vec::new(),
        extra: found
            .iter()
            .filter(|(name, _)| crate::model::is_field_name(name))
            .map(|(name, r)| (FieldKey::new(name), &message[r.clone()]))
            .collect(),
        kv_keys: pairs.iter().map(|(k, _)| FieldKey::new(&message[k.clone()])).collect(),
    })
}
