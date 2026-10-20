//! Column and value detection for Filter.
//!
//! When a file is loaded its columns and the distinct values of each column
//! are detected once. A column with more than [`MAX_DISTINCT`] distinct values
//! is marked as overflowing: it is not offered for filtering. `timestamp` and
//! `message`/payload are never filterable columns.

use crate::model::{FieldKey, LogEntry};
use crate::unicode_text;
use std::collections::HashMap;

/// A column with more distinct values than this cannot be filtered on.
pub const MAX_DISTINCT: usize = 500;

/// Value id of a row that has no value in a column.
pub const MISSING: u16 = u16::MAX;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Source {
    /// The Source File of a row in a Log Space (the Source Column).
    SourceFile,
    /// Which output of a Remote Command the line came from. A line's own:
    /// a Continuation Line does not take its Parent Line's.
    Stream,
    Level,
    Module,
    RequestId,
    TraceId,
    /// A field read from the line: a JSON key, or a Field Column's field.
    Field(FieldKey),
}

impl Source {
    fn key(&self) -> String {
        match self {
            Source::SourceFile => "source".to_string(),
            Source::Stream => "stream".to_string(),
            Source::Level => "level".to_string(),
            Source::Module => "module".to_string(),
            Source::RequestId => "request_id".to_string(),
            Source::TraceId => "trace_id".to_string(),
            Source::Field(k) => k.as_str().to_string(),
        }
    }

    fn label(&self) -> String {
        match self {
            Source::SourceFile => "Source".to_string(),
            Source::Stream => "Stream".to_string(),
            Source::Level => "Level".to_string(),
            Source::Module => "Module".to_string(),
            Source::RequestId => "Request ID".to_string(),
            Source::TraceId => "Trace ID".to_string(),
            Source::Field(k) => k.as_str().to_string(),
        }
    }

    /// Whether a Continuation Line takes its Parent Line's value.
    fn inherits(&self) -> bool {
        !matches!(self, Source::Stream)
    }

    fn value<'a>(&self, entry: &'a LogEntry) -> Option<&'a str> {
        let v = match self {
            Source::SourceFile => entry.source.as_deref().unwrap_or(""),
            Source::Stream => entry.stream.map_or("", |s| s.as_str()),
            Source::Level => entry.level_text.as_str(),
            Source::Module => entry.tag.as_str(),
            Source::RequestId => entry.request_id.as_deref().unwrap_or(""),
            Source::TraceId => entry.trace_id.as_deref().unwrap_or(""),
            Source::Field(k) => entry.span_text(k.as_str()).unwrap_or(""),
        };
        if v.is_empty() { None } else { Some(v) }
    }
}

/// The value of the row `row` in the column `key`. A Continuation Line has
/// no value of its own: it reads its Parent Line's, found by walking up to the
/// nearest row that is not a continuation. Its Stream is the exception: that
/// is always the line's own.
pub fn value_of<'a>(entries: &'a [LogEntry], row: usize, key: &str) -> Option<&'a str> {
    if key == "stream" {
        return entries.get(row).and_then(|e| column_value(e, key));
    }
    let mut r = row;
    while r > 0 && entries.get(r).is_some_and(|e| e.parent.is_some()) {
        r -= 1;
    }
    entries.get(r).and_then(|e| column_value(e, key))
}

/// The value a row holds in the column `key`, or `None` when it has none.
pub fn column_value<'a>(entry: &'a LogEntry, key: &str) -> Option<&'a str> {
    // Called once per row by Filter and Highlight Rules: no allocation here.
    let v = match key {
        "source" => entry.source.as_deref().unwrap_or(""),
        "stream" => entry.stream.map_or("", |s| s.as_str()),
        "message" => entry.payload.as_str(),
        "level" => entry.level_text.as_str(),
        "module" => entry.tag.as_str(),
        "request_id" => entry.request_id.as_deref().unwrap_or(""),
        "trace_id" => entry.trace_id.as_deref().unwrap_or(""),
        other => entry.span_text(other).unwrap_or(""),
    };
    if v.is_empty() { None } else { Some(v) }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnValue {
    /// Most frequent spelling of the value, shown in the checkbox.
    pub display: String,
    /// Folded (NFC, lowercase) form; the identity of the value.
    pub folded: String,
    /// Number of rows holding this value (case variants added together).
    pub count: usize,
}

#[derive(Debug, Clone)]
pub struct ColumnIndex {
    pub key: String,
    pub label: String,
    /// Distinct values in first-seen order. Empty when `overflow`.
    pub values: Vec<ColumnValue>,
    /// More than [`MAX_DISTINCT`] distinct values: not available for filtering.
    pub overflow: bool,
    source: Source,
    /// Value id per row (`MISSING` when absent). Empty when `overflow`.
    row_ids: Vec<u16>,
    folded_ids: HashMap<String, u16>,
    /// Raw spelling -> (id, rows), to pick the most frequent display form.
    spellings: HashMap<String, (u16, usize)>,
    rows: usize,
    /// Parent Lines that have a value in the column (Coverage). Kept when
    /// the column overflows.
    covered: usize,
}

impl ColumnIndex {
    fn new(source: Source, rows_before: usize) -> Self {
        Self {
            key: source.key(),
            label: source.label(),
            values: Vec::new(),
            overflow: false,
            source,
            row_ids: vec![MISSING; rows_before],
            folded_ids: HashMap::new(),
            spellings: HashMap::new(),
            rows: rows_before,
            covered: 0,
        }
    }

    /// The field the column shows, when it is a field read from the line.
    pub fn field(&self) -> Option<FieldKey> {
        match self.source {
            Source::Field(k) => Some(k),
            _ => None,
        }
    }

    /// Number of Parent Lines that have a value in the column.
    pub fn covered(&self) -> usize {
        self.covered
    }

    /// Number of distinct values, or `None` once the column overflowed.
    pub fn distinct_count(&self) -> Option<usize> {
        if self.overflow { None } else { Some(self.values.len()) }
    }

    /// Text for the "Add filter" column list: the count, or `> 500`.
    pub fn distinct_label(&self) -> String {
        match self.distinct_count() {
            Some(n) => n.to_string(),
            None => format!("> {}", MAX_DISTINCT),
        }
    }

    /// Whether the user may add conditions on this column.
    pub fn is_filterable(&self) -> bool {
        !self.overflow
    }

    /// Value id of `row`, or `None` when ids are not tracked for it.
    pub fn id_of_row(&self, row: usize) -> Option<u16> {
        self.row_ids.get(row).copied()
    }

    pub fn has_row_ids(&self) -> bool {
        !self.overflow && self.row_ids.len() == self.rows
    }

    /// Forget the rows from `keep` on; `removed` are those rows' entries.
    fn truncate(&mut self, keep: usize, removed: &[LogEntry]) {
        self.rows = keep;
        let lost = removed.iter().filter(|e| e.parent.is_none() && self.source.value(e).is_some()).count();
        self.covered = self.covered.saturating_sub(lost);
        if self.overflow {
            return;
        }
        for (i, entry) in removed.iter().enumerate() {
            if entry.parent.is_some() && self.source.inherits() {
                continue;
            }
            let Some(&id) = self.row_ids.get(keep + i) else { continue };
            if id == MISSING {
                continue;
            }
            self.values[id as usize].count -= 1;
            if let Some((_, n)) = self.source.value(entry).and_then(|raw| self.spellings.get_mut(raw)) {
                *n = n.saturating_sub(1);
            }
        }
        self.row_ids.truncate(keep);
    }

    fn push_row(&mut self, entry: &LogEntry, parent_row: usize) {
        self.rows += 1;
        if entry.parent.is_none() && self.source.value(entry).is_some() {
            self.covered += 1;
        }
        if self.overflow {
            return;
        }
        if entry.parent.is_some() && self.source.inherits() {
            // A Continuation Line holds its Parent Line's value, uncounted.
            let id = self.row_ids.get(parent_row).copied().unwrap_or(MISSING);
            self.row_ids.push(id);
            return;
        }
        let Some(raw) = self.source.value(entry) else {
            self.row_ids.push(MISSING);
            return;
        };
        if let Some((id, n)) = self.spellings.get_mut(raw) {
            *n += 1;
            let id = *id;
            self.values[id as usize].count += 1;
            self.row_ids.push(id);
            return;
        }
        let folded = unicode_text::fold(raw);
        let id = match self.folded_ids.get(&folded) {
            Some(&id) => id,
            None => {
                if self.values.len() >= MAX_DISTINCT {
                    self.mark_overflow();
                    return;
                }
                let id = self.values.len() as u16;
                self.values.push(ColumnValue {
                    display: raw.to_string(),
                    folded: folded.clone(),
                    count: 0,
                });
                self.folded_ids.insert(folded, id);
                id
            }
        };
        self.values[id as usize].count += 1;
        self.spellings.insert(raw.to_string(), (id, 1));
        self.row_ids.push(id);
    }

    /// Show each value in its most frequent spelling (ties: the smaller text).
    fn finalize_display(&mut self) {
        if self.overflow {
            return;
        }
        let mut best: HashMap<u16, (&str, usize)> = HashMap::new();
        for (spelling, (id, n)) in &self.spellings {
            let better = match best.get(id) {
                None => true,
                Some((cur, cur_n)) => *n > *cur_n || (*n == *cur_n && spelling.as_str() < *cur),
            };
            if better {
                best.insert(*id, (spelling.as_str(), *n));
            }
        }
        let chosen: Vec<(u16, String)> = best.into_iter().map(|(id, (s, _))| (id, s.to_string())).collect();
        for (id, s) in chosen {
            self.values[id as usize].display = s;
        }
    }

    fn mark_overflow(&mut self) {
        self.overflow = true;
        self.values = Vec::new();
        self.row_ids = Vec::new();
        self.folded_ids = HashMap::new();
        self.spellings = HashMap::new();
    }
}

/// Detected columns plus the timeline needed by the Time Range filter.
#[derive(Debug, Clone, Default)]
pub struct FilterIndex {
    pub columns: Vec<ColumnIndex>,
    rows: usize,
    /// Timestamp of each row, inherited from the nearest row above that has
    /// one; `NAN` before the first timestamp.
    effective_ts: Vec<f64>,
    first_ts: Option<f64>,
    min_entry: Option<(f64, String)>,
    max_entry: Option<(f64, String)>,
    /// Row of the latest line that is not a Continuation Line.
    parent_row: usize,
    /// Number of rows that are not Continuation Lines (Coverage's total).
    parent_lines: usize,
    /// Number of those with a timestamp of their own.
    timed_lines: usize,
    /// Parent Lines whose message has each `key=value` key, read or not.
    kv_counts: HashMap<FieldKey, usize>,
}

/// A row has its own timestamp when it is a finite, positive number.
pub fn has_timestamp(entry: &LogEntry) -> bool {
    entry.has_time && entry.timestamp.is_finite() && entry.timestamp > 0.0
}

impl FilterIndex {
    /// Detect the columns and values of a whole file.
    pub fn build(entries: &[LogEntry]) -> Self {
        let mut index = Self::default();
        index.append(entries);
        index
    }

    /// Detect the columns and values of a Log Space's merged rows.
    /// `field_order` is the union of the files' field names in column order,
    /// and `effective_ts` the timestamp each merged row sorts by (`NAN` for a
    /// row with none), which replaces the one inherited from the row above.
    pub fn build_merged(entries: &[LogEntry], field_order: &[String], effective_ts: &[f64]) -> Self {
        let mut index = Self::default();
        if !entries.is_empty() {
            index.init_fixed_columns();
            for name in field_order {
                index.columns.push(ColumnIndex::new(Source::Field(FieldKey::new(name)), 0));
            }
        }
        index.append(entries);
        if effective_ts.len() == index.rows {
            index.effective_ts = effective_ts.to_vec();
        }
        index
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    fn init_fixed_columns(&mut self) {
        // Fixed columns come first, in a stable order; empty ones are dropped
        // at the end of `columns_in_use`.
        for source in [
            Source::SourceFile,
            Source::Stream,
            Source::Level,
            Source::Module,
            Source::RequestId,
            Source::TraceId,
        ] {
            self.columns.push(ColumnIndex::new(source, 0));
        }
    }

    pub fn column(&self, key: &str) -> Option<&ColumnIndex> {
        self.columns.iter().find(|c| c.key == key)
    }

    /// Number of rows that are not Continuation Lines.
    pub fn parent_lines(&self) -> usize {
        self.parent_lines
    }

    /// Number of rows with a timestamp of their own.
    pub fn timed_lines(&self) -> usize {
        self.timed_lines
    }

    /// How many Parent Lines have each `key=value` key in their message.
    pub fn kv_counts(&self) -> &HashMap<FieldKey, usize> {
        &self.kv_counts
    }

    fn count_kv(&mut self, entry: &LogEntry, add: bool) {
        if entry.parent.is_some() {
            return;
        }
        for key in crate::extract::kv_keys(entry) {
            if add {
                *self.kv_counts.entry(key).or_default() += 1;
            } else if let Some(n) = self.kv_counts.get_mut(&key) {
                *n -= 1;
                if *n == 0 {
                    self.kv_counts.remove(&key);
                }
            }
        }
    }

    /// Coverage of the column `key`: the Parent Lines with a value in it, and
    /// all Parent Lines. A column the rows never had covers none.
    pub fn coverage(&self, key: &str) -> (usize, usize) {
        (self.column(key).map_or(0, |c| c.covered), self.parent_lines)
    }

    /// Detect the Field Columns again, the others kept: after the fields
    /// read from the rows changed (an Extraction Rule was added or changed).
    pub fn rebuild_fields(&mut self, entries: &[LogEntry]) {
        debug_assert_eq!(entries.len(), self.rows);
        self.columns.retain(|c| c.field().is_none());
        let mut fields: Vec<ColumnIndex> = Vec::new();
        let mut parent_row = 0;
        for (row, entry) in entries.iter().enumerate() {
            if entry.parent.is_none() {
                parent_row = row;
            }
            for (name, _) in entry.fields() {
                if !fields.iter().any(|c| c.source == Source::Field(name)) {
                    fields.push(ColumnIndex::new(Source::Field(name), row));
                }
            }
            for col in &mut fields {
                col.push_row(entry, parent_row);
            }
        }
        for col in &mut fields {
            col.finalize_display();
        }
        self.columns.extend(fields);
    }

    /// The fields read from the rows, as columns.
    pub fn field_columns(&self) -> impl Iterator<Item = &ColumnIndex> {
        self.columns.iter().filter(|c| c.field().is_some())
    }

    /// Account for rows added at the end of the file (tail): new values are
    /// appended after the existing ones and counts grow with the new rows.
    /// `new_entries` are the rows after the first `self.rows()` rows.
    pub fn append(&mut self, new_entries: &[LogEntry]) {
        for entry in new_entries {
            self.push_row(entry);
        }
        for col in &mut self.columns {
            col.finalize_display();
        }
    }

    /// Forget the rows after the first `kept.len()`; `removed` are the
    /// entries of those rows. They are meant to be added again with
    /// [`append`](Self::append), in a new order and with new rows among them,
    /// so the earliest and latest time are left as they were.
    pub fn truncate(&mut self, kept: &[LogEntry], removed: &[LogEntry]) {
        debug_assert_eq!(self.rows, kept.len() + removed.len());
        for col in &mut self.columns {
            col.truncate(kept.len(), removed);
        }
        self.rows = kept.len();
        self.effective_ts.truncate(kept.len());
        self.first_ts = self.effective_ts.iter().copied().find(|t| !t.is_nan());
        self.parent_row = kept.iter().rposition(|e| e.parent.is_none()).unwrap_or(0);
        self.parent_lines -= removed.iter().filter(|e| e.parent.is_none()).count();
        self.timed_lines -= removed.iter().filter(|e| e.parent.is_none() && has_timestamp(e)).count();
        for entry in removed {
            self.count_kv(entry, false);
        }
    }

    fn push_row(&mut self, entry: &LogEntry) {
        let row = self.rows;
        if self.columns.is_empty() && row == 0 {
            self.init_fixed_columns();
        }
        if entry.parent.is_none() {
            self.parent_row = row;
            self.parent_lines += 1;
            self.count_kv(entry, true);
        }
        let parent_row = self.parent_row;
        for (name, _) in entry.fields() {
            if !self.columns.iter().any(|c| c.source == Source::Field(name)) {
                self.columns.push(ColumnIndex::new(Source::Field(name), row));
            }
        }
        for col in &mut self.columns {
            col.push_row(entry, parent_row);
        }

        let prev = self.effective_ts.last().copied().unwrap_or(f64::NAN);
        if has_timestamp(entry) {
            if entry.parent.is_none() {
                self.timed_lines += 1;
            }
            self.effective_ts.push(entry.timestamp);
            if self.first_ts.is_none() {
                self.first_ts = Some(entry.timestamp);
            }
            if self.min_entry.as_ref().is_none_or(|(t, _)| entry.timestamp < *t) {
                self.min_entry = Some((entry.timestamp, entry.time_str.to_string()));
            }
            if self.max_entry.as_ref().is_none_or(|(t, _)| entry.timestamp > *t) {
                self.max_entry = Some((entry.timestamp, entry.time_str.to_string()));
            }
        } else {
            self.effective_ts.push(prev);
        }
        self.rows += 1;
    }

    /// Columns the file actually has: those with at least one value, plus any
    /// that overflowed (so they can be shown unavailable).
    pub fn columns_in_use(&self) -> impl Iterator<Item = &ColumnIndex> {
        self.columns
            .iter()
            .filter(|c| c.overflow || c.values.iter().any(|v| v.count > 0))
    }

    /// Timestamp of `row` as seen by the Time Range filter.
    pub fn effective_timestamp(&self, row: usize) -> Option<f64> {
        self.effective_ts.get(row).copied().filter(|t| !t.is_nan())
    }

    /// Start of the (naive UTC) day of the first timestamped row. A time of
    /// day typed without a date is read as being on this day.
    pub fn anchor_day(&self) -> f64 {
        self.first_ts.map_or(0.0, |t| (t / 86400.0).floor() * 86400.0)
    }

    /// Display forms of the earliest and latest timestamp, as hints.
    pub fn time_hint(&self) -> Option<(String, String)> {
        match (&self.min_entry, &self.max_entry) {
            (Some((_, a)), Some((_, b))) => Some((a.clone(), b.clone())),
            _ => None,
        }
    }
}
