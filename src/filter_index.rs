//! Column and value detection for Filter.
//!
//! When a file is loaded its columns and the distinct values of each column
//! are detected once. A column with more than [`MAX_DISTINCT`] distinct values
//! is marked as overflowing: it is not offered for filtering. `timestamp` and
//! `message`/payload are never filterable columns.

use crate::model::LogEntry;
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
    Level,
    Module,
    RequestId,
    TraceId,
    Json(String),
}

impl Source {
    fn key(&self) -> String {
        match self {
            Source::SourceFile => "source".to_string(),
            Source::Level => "level".to_string(),
            Source::Module => "module".to_string(),
            Source::RequestId => "request_id".to_string(),
            Source::TraceId => "trace_id".to_string(),
            Source::Json(k) => k.clone(),
        }
    }

    fn label(&self) -> String {
        match self {
            Source::SourceFile => "Source".to_string(),
            Source::Level => "Level".to_string(),
            Source::Module => "Module".to_string(),
            Source::RequestId => "Request ID".to_string(),
            Source::TraceId => "Trace ID".to_string(),
            Source::Json(k) => k.clone(),
        }
    }

    fn value<'a>(&self, entry: &'a LogEntry) -> Option<&'a str> {
        let v = match self {
            Source::SourceFile => entry.source.as_deref().unwrap_or(""),
            Source::Level => {
                if entry.level == crate::model::LogLevel::Unknown {
                    return None;
                }
                entry.level.as_str()
            }
            Source::Module => entry.tag.as_str(),
            Source::RequestId => entry.request_id.as_deref().unwrap_or(""),
            Source::TraceId => entry.trace_id.as_deref().unwrap_or(""),
            Source::Json(k) => entry
                .json_fields
                .iter()
                .find(|(name, _)| name == k)
                .map(|(_, v)| v.as_str())
                .unwrap_or(""),
        };
        if v.is_empty() { None } else { Some(v) }
    }
}

/// The value a row holds in the column `key`, or `None` when it has none.
pub fn column_value<'a>(entry: &'a LogEntry, key: &str) -> Option<&'a str> {
    let source = match key {
        "source" => Source::SourceFile,
        "level" => Source::Level,
        "module" => Source::Module,
        "request_id" => Source::RequestId,
        "trace_id" => Source::TraceId,
        other => Source::Json(other.to_string()),
    };
    source.value(entry)
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
        }
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

    fn push_row(&mut self, entry: &LogEntry) {
        self.rows += 1;
        if self.overflow {
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
                index.columns.push(ColumnIndex::new(Source::Json(name.clone()), 0));
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

    fn push_row(&mut self, entry: &LogEntry) {
        let row = self.rows;
        if self.columns.is_empty() && row == 0 {
            self.init_fixed_columns();
        }
        for (name, _) in &entry.json_fields {
            if !self.columns.iter().any(|c| c.source == Source::Json(name.clone())) {
                self.columns.push(ColumnIndex::new(Source::Json(name.clone()), row));
            }
        }
        for col in &mut self.columns {
            col.push_row(entry);
        }

        let prev = self.effective_ts.last().copied().unwrap_or(f64::NAN);
        if has_timestamp(entry) {
            self.effective_ts.push(entry.timestamp);
            if self.first_ts.is_none() {
                self.first_ts = Some(entry.timestamp);
            }
            if self.min_entry.as_ref().is_none_or(|(t, _)| entry.timestamp < *t) {
                self.min_entry = Some((entry.timestamp, entry.time_str.clone()));
            }
            if self.max_entry.as_ref().is_none_or(|(t, _)| entry.timestamp > *t) {
                self.max_entry = Some((entry.timestamp, entry.time_str.clone()));
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
