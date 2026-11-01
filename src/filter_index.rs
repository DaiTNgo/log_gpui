//! Column and value detection for Filter.
//!
//! When a file is loaded its columns and the distinct values of each column
//! are detected once. A column with more than [`MAX_DISTINCT`] distinct values
//! is marked as overflowing: it is not offered for filtering. `timestamp` and
//! `message`/payload are never filterable columns.

use crate::model::{FieldKey, LogEntry};
use crate::rows::Rows;
use crate::unicode_text;
use std::collections::{HashMap, HashSet};

mod chunk;
mod merge;
mod view;
pub use chunk::{ChunkColumns, LineFacts, Overflowed};
pub use merge::{ColumnMap, MergeFile};
pub use view::{RowIds, RowTable, SpaceIndex, ViewIndex, ViewIndexRead};

/// A column with more distinct values than this cannot be filtered on.
pub const MAX_DISTINCT: usize = 1000;

/// Value id of a row that has no value in a column.
pub const MISSING: u16 = u16::MAX;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Source {
    /// The Source File of a row in a Log Space (the Source Column).
    SourceFile,
    /// The Log Name of a row's Source File in a Log Space (the Log Name
    /// Column): its name without a rotation suffix.
    LogName,
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
            Source::LogName => crate::columns::LOG_NAME_KEY.to_string(),
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
            Source::LogName => "Log Name".to_string(),
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

    /// The value of `entry`, whose Source File is named `source`.
    fn value<'a>(&self, entry: &'a LogEntry, source: Option<&'a str>) -> Option<&'a str> {
        let v = match self {
            Source::SourceFile => source.unwrap_or(""),
            Source::LogName => source.map_or("", crate::rotation_name::log_name),
            Source::Stream => entry.stream.map_or("", |s| s.as_str()),
            Source::Level => entry.level_text(),
            Source::Module => entry.tag(),
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
/// is always the line's own. `f` is given the value.
pub fn with_value_of<R: Rows + ?Sized, T>(rows: &R, row: usize, key: &str, f: impl FnOnce(Option<&str>) -> T) -> T {
    let mut r = row;
    if key != "stream" {
        while r > 0 && rows.get(r).is_some_and(|e| e.parent.is_some()) {
            r -= 1;
        }
    }
    if key == "source" {
        return f(rows.source(r).filter(|s| !s.is_empty()));
    }
    if key == crate::columns::LOG_NAME_KEY {
        return f(rows.source(r).map(crate::rotation_name::log_name).filter(|s| !s.is_empty()));
    }
    match rows.get(r) {
        Some(entry) => f(column_value(&entry, key)),
        None => f(None),
    }
}

/// The value a row holds in the column `key`, or `None` when it has none.
pub fn column_value<'a>(entry: &'a LogEntry, key: &str) -> Option<&'a str> {
    // Called once per row by Filter and Highlight Rules: no allocation here.
    let v = match key {
        "source" => entry.source.as_deref().unwrap_or(""),
        crate::columns::LOG_NAME_KEY => entry.source.as_deref().map_or("", crate::rotation_name::log_name),
        "stream" => entry.stream.map_or("", |s| s.as_str()),
        "message" => entry.payload(),
        "level" => entry.level_text(),
        "module" => entry.tag(),
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

/// The value id of each row of a column. The rows before the first that
/// has a value hold no id: a column no row has a value in keeps nothing
/// per row (lean-view-memory, decision 7).
#[derive(Debug, Clone, Default)]
pub(crate) struct RowIdList {
    /// Rows before the first id kept: all `MISSING`.
    from: usize,
    ids: Vec<u16>,
}

impl RowIdList {
    /// `rows` rows with no value.
    fn missing(rows: usize) -> Self {
        Self { from: rows, ids: Vec::new() }
    }

    pub(crate) fn len(&self) -> usize {
        self.from + self.ids.len()
    }

    pub(crate) fn get(&self, row: usize) -> Option<u16> {
        if row < self.from {
            Some(MISSING)
        } else {
            self.ids.get(row - self.from).copied()
        }
    }

    pub(crate) fn push(&mut self, id: u16) {
        if self.ids.is_empty() && id == MISSING {
            self.from += 1;
        } else {
            self.ids.push(id);
        }
    }

    fn set(&mut self, row: usize, id: u16) {
        if row >= self.len() {
            self.resize(row + 1);
        }
        if row < self.from {
            if id == MISSING {
                return;
            }
            let mut ids = vec![MISSING; self.from - row];
            ids.append(&mut self.ids);
            self.ids = ids;
            self.from = row;
        }
        self.ids[row - self.from] = id;
    }

    /// As many rows as `rows`, the new ones with no value.
    fn resize(&mut self, rows: usize) {
        if rows <= self.from {
            self.from = rows;
            self.ids = Vec::new();
        } else if self.ids.is_empty() {
            self.from = rows;
        } else {
            self.ids.resize(rows - self.from, MISSING);
        }
    }

    fn truncate(&mut self, rows: usize) {
        if rows < self.len() {
            self.resize(rows);
        }
    }

    /// Forget the rows at `dropped` (ascending).
    fn remove(&mut self, dropped: &[usize]) {
        let before = dropped.partition_point(|&r| r < self.from);
        let kept: Vec<usize> = dropped[before..].iter().map(|&r| r - self.from).collect();
        self.from -= before;
        remove_positions(&mut self.ids, &kept);
    }

    fn heap_bytes(&self) -> usize {
        self.ids.capacity() * 2
    }
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
    row_ids: RowIdList,
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
            row_ids: RowIdList::missing(rows_before),
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

    /// Text for the "Add filter" column list: the count, or `> 1000`.
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
        self.row_ids.get(row)
    }

    pub fn has_row_ids(&self) -> bool {
        !self.overflow && self.row_ids.len() == self.rows
    }

    /// Forget the rows from `keep` on; `removed` are those rows.
    fn truncate<R: Rows + ?Sized>(&mut self, keep: usize, removed: &R) {
        self.rows = keep;
        let lost = (0..removed.len())
            .filter(|&i| removed.get(i).is_some_and(|e| e.parent.is_none() && self.source.value(&e, removed.source(i)).is_some()))
            .count();
        self.covered = self.covered.saturating_sub(lost);
        if self.overflow {
            return;
        }
        for i in 0..removed.len() {
            let Some(entry) = removed.get(i) else { continue };
            if entry.parent.is_some() && self.source.inherits() {
                continue;
            }
            let Some(id) = self.row_ids.get(keep + i) else { continue };
            if id == MISSING {
                continue;
            }
            self.values[id as usize].count -= 1;
            if let Some((_, n)) = self.source.value(&entry, removed.source(i)).and_then(|raw| self.spellings.get_mut(raw)) {
                *n = n.saturating_sub(1);
            }
        }
        self.row_ids.truncate(keep);
    }

    /// Forget the rows at `dropped` (ascending positions; `removed` are
    /// those rows, in the same order). The other rows keep their order.
    fn drop_rows<R: Rows + ?Sized>(&mut self, dropped: &[usize], removed: &R) {
        let lost = (0..removed.len())
            .filter(|&i| removed.get(i).is_some_and(|e| e.parent.is_none() && self.source.value(&e, removed.source(i)).is_some()))
            .count();
        self.covered = self.covered.saturating_sub(lost);
        self.rows -= dropped.len();
        if self.overflow {
            return;
        }
        for i in 0..removed.len() {
            let Some(entry) = removed.get(i) else { continue };
            if entry.parent.is_some() && self.source.inherits() {
                continue;
            }
            let Some(id) = self.row_ids.get(dropped[i]) else { continue };
            if id == MISSING {
                continue;
            }
            self.values[id as usize].count -= 1;
            if let Some((_, n)) = self.source.value(&entry, removed.source(i)).and_then(|raw| self.spellings.get_mut(raw)) {
                *n = n.saturating_sub(1);
            }
        }
        self.row_ids.remove(dropped);
    }

    fn push_row(&mut self, entry: &LogEntry, source: Option<&str>, parent_row: usize) {
        self.rows += 1;
        if entry.parent.is_none() && self.source.value(entry, source).is_some() {
            self.covered += 1;
        }
        if self.overflow {
            return;
        }
        if entry.parent.is_some() && self.source.inherits() {
            // A Continuation Line holds its Parent Line's value, uncounted.
            let id = self.row_ids.get(parent_row).unwrap_or(MISSING);
            self.row_ids.push(id);
            return;
        }
        let Some(raw) = self.source.value(entry, source) else {
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
        self.row_ids = RowIdList::default();
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
    /// one; `NAN` before the first timestamp. Shared, so a count on a
    /// worker thread reads it without a copy.
    effective_ts: std::sync::Arc<Vec<f64>>,
    first_ts: Option<f64>,
    /// The row `first_ts` is the time of: the rows before it have none.
    first_ts_row: Option<usize>,
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
    /// The fields that have a column (a lookup, not a scan of `columns`).
    field_cols: HashSet<FieldKey>,
}

/// A row has its own timestamp when it is a finite, positive number.
pub fn has_timestamp(entry: &LogEntry) -> bool {
    entry.has_time && entry.timestamp.is_finite() && entry.timestamp > 0.0
}

impl FilterIndex {
    /// The memory it holds on the heap, in bytes, roughly: the row ids and
    /// times of every row, and the values of each column.
    pub fn heap_bytes(&self) -> usize {
        // A map entry: its key, its value and the table's own share.
        const ENTRY: usize = 48;
        let columns: usize = self
            .columns
            .iter()
            .map(|c| {
                let values: usize =
                    c.values.iter().map(|v| std::mem::size_of::<ColumnValue>() + v.display.capacity() + v.folded.capacity()).sum();
                c.row_ids.heap_bytes() + values + (c.folded_ids.len() + c.spellings.len()) * ENTRY
            })
            .sum();
        self.effective_ts.capacity() * 8 + columns + (self.kv_counts.len() + self.field_cols.len()) * ENTRY
    }

    /// The rows `rows` of a Remote Command read from disk came from these
    /// outputs (`stream_of`), which Indexing could not know: their Stream
    /// was missing until now. `is_parent` says which are Parent Lines, for
    /// Coverage.
    pub fn set_streams(
        &mut self,
        rows: std::ops::Range<usize>,
        stream_of: impl Fn(usize) -> Option<crate::model::Stream>,
        is_parent: impl Fn(usize) -> bool,
    ) {
        let Some(col) = self.columns.iter_mut().find(|c| matches!(c.source, Source::Stream)) else { return };
        if col.overflow {
            return;
        }
        col.row_ids.resize(col.rows);
        for row in rows {
            let Some(stream) = stream_of(row) else { continue };
            if col.row_ids.get(row) != Some(MISSING) {
                continue;
            }
            let raw = stream.as_str();
            let id = match col.folded_ids.get(raw) {
                Some(&id) => id,
                None => {
                    let id = col.values.len() as u16;
                    col.values.push(ColumnValue { display: raw.to_string(), folded: raw.to_string(), count: 0 });
                    col.folded_ids.insert(raw.to_string(), id);
                    id
                }
            };
            col.values[id as usize].count += 1;
            col.spellings.entry(raw.to_string()).or_insert((id, 0)).1 += 1;
            col.row_ids.set(row, id);
            if is_parent(row) {
                col.covered += 1;
            }
        }
    }

    /// Detect the columns and values of a whole file.
    pub fn build<R: Rows + ?Sized>(rows: &R) -> Self {
        let mut index = Self::default();
        index.append(rows);
        index
    }

    /// Detect the columns and values of a Log Space's merged rows.
    /// `field_order` is the union of the files' field names in column order,
    /// and `effective_ts` the timestamp each merged row sorts by (`NAN` for a
    /// row with none), which replaces the one inherited from the row above.
    pub fn build_merged<R: Rows + ?Sized>(entries: &R, field_order: &[String], effective_ts: &[f64]) -> Self {
        let mut index = Self::default();
        if !entries.is_empty() {
            index.init_fixed_columns();
            for name in field_order {
                let key = FieldKey::new(name);
                index.field_cols.insert(key);
                index.columns.push(ColumnIndex::new(Source::Field(key), 0));
            }
        }
        index.append(entries);
        if effective_ts.len() == index.rows {
            index.effective_ts = std::sync::Arc::new(effective_ts.to_vec());
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
            Source::LogName,
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
    /// read from the rows changed (a Detected Field was turned on or off).
    pub fn rebuild_fields<R: Rows + ?Sized>(&mut self, rows: &R) {
        debug_assert_eq!(rows.len(), self.rows);
        self.columns.retain(|c| c.field().is_none());
        let mut fields: Vec<ColumnIndex> = Vec::new();
        self.field_cols.clear();
        let mut parent_row = 0;
        for row in 0..rows.len() {
            let Some(entry) = rows.get(row) else { continue };
            if entry.parent.is_none() {
                parent_row = row;
            }
            for (name, _) in entry.fields() {
                if self.field_cols.insert(name) {
                    fields.push(ColumnIndex::new(Source::Field(name), row));
                }
            }
            for col in &mut fields {
                col.push_row(&entry, rows.source(row), parent_row);
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
    pub fn append<R: Rows + ?Sized>(&mut self, new_rows: &R) {
        for row in 0..new_rows.len() {
            if let Some(entry) = new_rows.get(row) {
                self.push_row(&entry, new_rows.source(row));
            }
        }
        for col in &mut self.columns {
            col.finalize_display();
        }
    }

    /// Forget the rows after the first `kept.len()`; `removed` are the
    /// entries of those rows. They are meant to be added again with
    /// [`append`](Self::append), in a new order and with new rows among them,
    /// so the earliest and latest time are left as they were.
    pub fn truncate<K: Rows + ?Sized, R: Rows + ?Sized>(&mut self, kept: &K, removed: &R) {
        let parent_row = (0..kept.len()).rev().find(|&r| kept.get(r).is_some_and(|e| e.parent.is_none())).unwrap_or(0);
        self.truncate_to(kept.len(), parent_row, removed);
    }

    /// `truncate` to the first `keep` rows, of which the last that is not a
    /// Continuation Line is `parent_row`.
    pub fn truncate_to<R: Rows + ?Sized>(&mut self, keep: usize, parent_row: usize, removed: &R) {
        debug_assert_eq!(self.rows, keep + removed.len());
        for col in &mut self.columns {
            col.truncate(keep, removed);
        }
        self.rows = keep;
        if !self.effective_ts.is_empty() {
            std::sync::Arc::make_mut(&mut self.effective_ts).truncate(keep);
        }
        if self.first_ts_row.is_some_and(|r| r >= keep) {
            self.first_ts = None;
            self.first_ts_row = None;
        }
        self.parent_row = parent_row;
        self.forget_counts(removed);
    }

    /// Take the rows `removed` out of the line counts and `kv_counts`.
    fn forget_counts<R: Rows + ?Sized>(&mut self, removed: &R) {
        for entry in crate::rows::entries(removed) {
            if entry.parent.is_none() {
                self.parent_lines -= 1;
                if has_timestamp(&entry) {
                    self.timed_lines -= 1;
                }
            }
            self.count_kv(&entry, false);
        }
    }

    /// Forget the rows at `dropped` (ascending positions); `removed` are
    /// those rows' entries, in the same order, and `kept` the rows left.
    /// Rows from the front of the view leave when the Live Window trims a
    /// file.
    pub fn drop_rows<R: Rows + ?Sized, K: Rows + ?Sized>(&mut self, dropped: &[usize], removed: &R, kept: &K) {
        debug_assert_eq!(self.rows, kept.len() + removed.len());
        for col in &mut self.columns {
            col.drop_rows(dropped, removed);
            col.finalize_display();
        }
        let parent_at_or_before = self.parent_row;
        let before = dropped.partition_point(|&p| p <= parent_at_or_before);
        self.parent_row = parent_at_or_before.saturating_sub(before);
        self.rows = kept.len();
        remove_positions(std::sync::Arc::make_mut(&mut self.effective_ts), dropped);
        self.first_ts = self.effective_ts.iter().copied().find(|t| !t.is_nan());
        self.forget_counts(removed);
        // The earliest and latest time are those of the rows left (rows are
        // in time order).
        let timed = |r: &usize| kept.get(*r).filter(|e| has_timestamp(e));
        if let Some(first) = (0..kept.len()).find_map(|r| timed(&r)) {
            self.min_entry = Some((first.timestamp, first.time_str().to_string()));
        }
        if let Some(last) = (0..kept.len()).rev().find_map(|r| timed(&r)) {
            self.max_entry = Some((last.timestamp, last.time_str().to_string()));
        }
        if kept.is_empty() {
            self.min_entry = None;
            self.max_entry = None;
        }
    }

    fn push_row(&mut self, entry: &LogEntry, source: Option<&str>) {
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
            if self.field_cols.insert(name) {
                self.columns.push(ColumnIndex::new(Source::Field(name), row));
            }
        }
        for col in &mut self.columns {
            col.push_row(entry, source, parent_row);
        }

        let prev = self.effective_ts.last().copied().unwrap_or(f64::NAN);
        if has_timestamp(entry) {
            if entry.parent.is_none() {
                self.timed_lines += 1;
            }
            std::sync::Arc::make_mut(&mut self.effective_ts).push(entry.timestamp);
            if self.first_ts.is_none() {
                self.first_ts = Some(entry.timestamp);
                self.first_ts_row = Some(self.effective_ts.len() - 1);
            }
            if self.min_entry.as_ref().is_none_or(|(t, _)| entry.timestamp < *t) {
                self.min_entry = Some((entry.timestamp, entry.time_str().to_string()));
            }
            if self.max_entry.as_ref().is_none_or(|(t, _)| entry.timestamp > *t) {
                self.max_entry = Some((entry.timestamp, entry.time_str().to_string()));
            }
        } else {
            std::sync::Arc::make_mut(&mut self.effective_ts).push(prev);
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

    /// The row of the first timestamp: the rows before it have none.
    pub fn first_timed_row(&self) -> Option<usize> {
        self.first_ts_row
    }

    /// The earliest and latest timestamp of the rows.
    pub fn time_span(&self) -> Option<(f64, f64)> {
        Some((self.min_entry.as_ref()?.0, self.max_entry.as_ref()?.0))
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

/// Remove the items at `dropped` (ascending positions) from `items`.
pub(crate) fn remove_positions<T>(items: &mut Vec<T>, dropped: &[usize]) {
    let (Some(&first), Some(&last)) = (dropped.first(), dropped.last()) else { return };
    if last - first + 1 == dropped.len() {
        // One run, the usual case: the oldest lines of one file.
        let end = (last + 1).min(items.len());
        items.drain(first.min(end)..end);
        return;
    }
    let mut next = 0;
    let mut row = 0;
    items.retain(|_| {
        let drop = next < dropped.len() && dropped[next] == row;
        if drop {
            next += 1;
        }
        row += 1;
        !drop
    });
}


#[cfg(test)]
mod tests {
    use super::{MISSING, RowIdList};

    fn ids(list: &RowIdList) -> Vec<u16> {
        (0..list.len()).map(|r| list.get(r).unwrap()).collect()
    }

    #[test]
    fn rows_with_no_value_keep_nothing() {
        let mut list = RowIdList::missing(3);
        for _ in 0..1000 {
            list.push(MISSING);
        }
        assert_eq!((list.len(), list.heap_bytes()), (1003, 0));
        assert_eq!(list.get(500), Some(MISSING));
        assert_eq!(list.get(1003), None);
        list.push(7);
        list.push(MISSING);
        assert_eq!(list.len(), 1005);
        assert_eq!((list.get(1002), list.get(1003), list.get(1004)), (Some(MISSING), Some(7), Some(MISSING)));
    }

    #[test]
    fn a_value_set_before_the_first_kept_one_keeps_the_rows_from_it() {
        let mut list = RowIdList::missing(5);
        list.push(1);
        list.set(2, 4);
        assert_eq!(ids(&list), [MISSING, MISSING, 4, MISSING, MISSING, 1]);
        list.set(8, 2);
        assert_eq!(ids(&list), [MISSING, MISSING, 4, MISSING, MISSING, 1, MISSING, MISSING, 2]);
    }

    #[test]
    fn rows_are_cut_and_dropped_across_the_first_kept_one() {
        let mut list = RowIdList::missing(4);
        for id in [1, 2, 3] {
            list.push(id);
        }
        list.remove(&[0, 1, 4]);
        assert_eq!(ids(&list), [MISSING, MISSING, 2, 3]);
        list.truncate(3);
        assert_eq!(ids(&list), [MISSING, MISSING, 2]);
        list.truncate(1);
        assert_eq!((ids(&list), list.heap_bytes()), (vec![MISSING], 0));
        list.resize(4);
        assert_eq!(ids(&list), [MISSING; 4]);
    }
}
