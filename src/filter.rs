//! Filter: narrowing the shown lines by the values of the file's columns.
//!
//! Per column the user ticks Filter Values (equality) or adds Custom Values
//! (contains); both combine with OR inside a column, columns and the Time
//! Range combine with AND. Hide Mode flips a column from "show only these" to
//! "hide these". A row with no value in a column never matches it.

use crate::filter_index::{ColumnIndex, FilterIndex, MISSING, value_of};
use crate::model::LogEntry;
use crate::unicode_text::{self, Needle};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnFilter {
    /// Column key (`level`, `module`, `request_id`, `trace_id` or a JSON field).
    pub column: String,
    /// Ticked Filter Values, in folded form.
    #[serde(default)]
    pub values: Vec<String>,
    /// Custom Values: rows whose value contains the text.
    #[serde(default)]
    pub custom: Vec<String>,
    /// Hide the matching rows instead of showing only them.
    #[serde(default)]
    pub hide: bool,
}

impl ColumnFilter {
    fn new(column: &str) -> Self {
        Self { column: column.to_string(), ..Self::default() }
    }

    /// Whether this column has any condition to apply.
    pub fn is_active(&self) -> bool {
        !self.values.is_empty() || !self.custom.is_empty()
    }

    pub fn has_value(&self, folded: &str) -> bool {
        self.values.iter().any(|v| v == folded)
    }
}

/// From/To bounds on `timestamp`, as typed. An empty bound is unbounded.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeRange {
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub to: String,
}

impl TimeRange {
    pub fn is_active(&self) -> bool {
        !self.from.trim().is_empty() || !self.to.trim().is_empty()
    }

    /// Resolve both bounds; a bound that does not parse is treated as unbounded.
    pub fn bounds(&self, anchor_day: f64) -> (Option<f64>, Option<f64>) {
        (
            parse_time_bound(&self.from, anchor_day).ok().flatten(),
            parse_time_bound(&self.to, anchor_day).ok().flatten(),
        )
    }
}

/// A typed time bound that could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidTime;

/// Parse a typed time bound in the form the Timestamp column shows.
///
/// Accepts a full date and time (`2026-10-17 10:42:03.250`, `T` or `/`
/// separators), a time of day (`10:42:03`, `10:42`, read as on `anchor_day`),
/// or a plain number of seconds. Times are taken on the file's own clock, with
/// no timezone conversion. Empty text is `Ok(None)`.
pub fn parse_time_bound(text: &str, anchor_day: f64) -> Result<Option<f64>, InvalidTime> {
    use chrono::{NaiveDateTime, NaiveTime, Timelike};
    let t = text.trim();
    if t.is_empty() {
        return Ok(None);
    }
    if let Ok(n) = t.parse::<f64>() {
        return if n.is_finite() { Ok(Some(n)) } else { Err(InvalidTime) };
    }
    if let Some(secs) = crate::parser::text::read_timestamp(t) {
        return Ok(Some(secs));
    }
    let t = t.trim_end_matches('Z');
    for fmt in ["%Y-%m-%d %H:%M", "%Y-%m-%dT%H:%M"] {
        if let Ok(dt) = NaiveDateTime::parse_from_str(t, fmt) {
            let utc = dt.and_utc();
            return Ok(Some(utc.timestamp() as f64 + f64::from(utc.timestamp_subsec_millis()) / 1000.0));
        }
    }
    for fmt in ["%H:%M:%S%.f", "%H:%M"] {
        if let Ok(tod) = NaiveTime::parse_from_str(t, fmt) {
            let secs = f64::from(tod.num_seconds_from_midnight()) + f64::from(tod.nanosecond()) / 1e9;
            return Ok(Some(anchor_day + secs));
        }
    }
    Err(InvalidTime)
}

/// The Filter of one log file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilterState {
    #[serde(default)]
    pub columns: Vec<ColumnFilter>,
    #[serde(default)]
    pub time_range: TimeRange,
}

impl FilterState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        !self.time_range.is_active() && self.columns.iter().all(|c| !c.is_active())
    }

    /// Number of active conditions: columns with a condition, plus the Time Range.
    pub fn active_count(&self) -> usize {
        self.columns.iter().filter(|c| c.is_active()).count()
            + usize::from(self.time_range.is_active())
    }

    pub fn column(&self, key: &str) -> Option<&ColumnFilter> {
        self.columns.iter().find(|c| c.column == key)
    }

    fn column_mut(&mut self, key: &str) -> &mut ColumnFilter {
        if let Some(pos) = self.columns.iter().position(|c| c.column == key) {
            return &mut self.columns[pos];
        }
        self.columns.push(ColumnFilter::new(key));
        self.columns.last_mut().unwrap()
    }

    fn prune(&mut self) {
        self.columns.retain(|c| c.is_active() || c.hide);
    }

    /// Whether the Filter Value `folded` of `key` is ticked.
    pub fn is_value_ticked(&self, key: &str, folded: &str) -> bool {
        self.column(key).is_some_and(|c| c.has_value(folded))
    }

    /// Tick or untick a Filter Value (given in any case; stored folded).
    pub fn toggle_value(&mut self, key: &str, value: &str) {
        let folded = unicode_text::fold(value);
        let col = self.column_mut(key);
        if let Some(pos) = col.values.iter().position(|v| *v == folded) {
            col.values.remove(pos);
        } else {
            col.values.push(folded);
        }
        self.prune();
    }

    /// Add a Custom Value (contains). Blank or duplicate text is ignored.
    pub fn add_custom(&mut self, key: &str, text: &str) -> bool {
        let text = text.trim();
        if text.is_empty() {
            return false;
        }
        let folded = unicode_text::fold(text);
        let col = self.column_mut(key);
        if col.custom.iter().any(|c| unicode_text::fold(c) == folded) {
            return false;
        }
        col.custom.push(text.to_string());
        true
    }

    pub fn remove_custom(&mut self, key: &str, text: &str) {
        if let Some(col) = self.columns.iter_mut().find(|c| c.column == key) {
            col.custom.retain(|c| c != text);
        }
        self.prune();
    }

    pub fn set_hide(&mut self, key: &str, hide: bool) {
        self.column_mut(key).hide = hide;
        self.prune();
    }

    /// Remove every condition of one column.
    pub fn clear_column(&mut self, key: &str) {
        self.columns.retain(|c| c.column != key);
    }

    pub fn set_time_range(&mut self, from: &str, to: &str) {
        self.time_range = TimeRange { from: from.trim().to_string(), to: to.trim().to_string() };
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Evaluate the Filter and return the indices of the rows that pass it.
    /// Also refreshes every row's delta time from the row above it: Filter
    /// narrows the Focus Pane, not the view.
    pub fn evaluate(&self, index: &FilterIndex, entries: &mut [LogEntry]) -> Vec<usize> {
        let mut visible = Vec::with_capacity(entries.len());
        let mut prev_ts: Option<f64> = None;

        let stale = index.rows() != entries.len();
        let active: Vec<(&ColumnFilter, CompiledColumn)> = self
            .columns
            .iter()
            .filter(|c| c.is_active())
            .map(|c| (c, CompiledColumn::compile(c, index.column(&c.column), stale)))
            .collect();
        let (from, to) = self.time_range.bounds(index.anchor_day());
        let time_active = (from.is_some() || to.is_some()) && !stale;

        for row in 0..entries.len() {
            let columns_ok = active.iter().all(|(cf, compiled)| {
                let matched = compiled.matches(row, entries, &cf.column);
                matched != cf.hide
            });
            let time_ok = !time_active
                || match index.effective_timestamp(row) {
                    None => true,
                    Some(ts) => from.is_none_or(|f| ts >= f) && to.is_none_or(|t| ts <= t),
                };
            if columns_ok && time_ok {
                visible.push(row);
            }
        }
        // The view shows every row, so each delta is from the row above it.
        for entry in entries.iter_mut() {
            update_entry_delta_time(entry, &mut prev_ts);
        }
        visible
    }
}

/// A column's condition prepared for fast evaluation. With value ids available
/// it is a lookup table over the (at most 500) values; otherwise it compares
/// the row's text directly.
struct CompiledColumn<'a> {
    /// Set only when the column has value ids that match the entries.
    ids: Option<&'a ColumnIndex>,
    table: Vec<bool>,
    values: Vec<Needle>,
    custom: Vec<Needle>,
}

impl<'a> CompiledColumn<'a> {
    fn compile(cf: &ColumnFilter, col: Option<&'a ColumnIndex>, stale: bool) -> Self {
        let values: Vec<Needle> = cf.values.iter().map(|v| Needle::new(v)).collect();
        let custom: Vec<Needle> = cf.custom.iter().map(|c| Needle::new(c)).collect();
        let mut ids = None;
        let mut table = Vec::new();
        if let (Some(col), false) = (col, stale)
            && col.has_row_ids()
        {
            let ticked: HashSet<&str> = cf.values.iter().map(String::as_str).collect();
            table = col
                .values
                .iter()
                .map(|v| {
                    ticked.contains(v.folded.as_str()) || custom.iter().any(|n| n.is_in(&v.display))
                })
                .collect();
            ids = Some(col);
        }
        Self { ids, table, values, custom }
    }

    fn matches(&self, row: usize, entries: &[LogEntry], key: &str) -> bool {
        if let Some(col) = self.ids {
            return match col.id_of_row(row) {
                Some(MISSING) => false,
                Some(id) => self.table.get(id as usize).copied().unwrap_or(false),
                None => self.matches_text(entries, row, key),
            };
        }
        self.matches_text(entries, row, key)
    }

    fn matches_text(&self, entries: &[LogEntry], row: usize, key: &str) -> bool {
        match value_of(entries, row, key) {
            None => false,
            Some(v) => self.values.iter().any(|n| n.equals(v)) || self.custom.iter().any(|n| n.is_in(v)),
        }
    }
}

fn update_entry_delta_time(entry: &mut LogEntry, prev_ts: &mut Option<f64>) {
    if entry.parent.is_some() {
        // A Continuation Line has no time of its own.
        entry.delta_time_ms = None;
        return;
    }
    if let Some(prev) = *prev_ts {
        entry.delta_time_ms = Some((entry.timestamp - prev) * 1000.0);
    } else {
        entry.delta_time_ms = Some(0.0);
    }
    *prev_ts = Some(entry.timestamp);
}
