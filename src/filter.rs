//! Filter: narrowing the shown lines by the values of the file's columns.
//!
//! Per column the user ticks Filter Values (equality) or adds Custom Values
//! (contains); both combine with OR inside a column, columns and the Time
//! Range combine with AND. Hide Mode flips a column from "show only these" to
//! "hide these". A row with no value in a column never matches it.

use crate::filter_index::{ColumnIndex, FilterIndex, MISSING, with_value_of};
use crate::rows::Rows;
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

    /// A one-line summary of the column's condition, as its Filter card
    /// shows it: `level: error, warn`, `not module: DB`, `message: contains “timeout”`;
    /// past two values, `+N`.
    /// `display` gives a Filter Value as the column shows it.
    pub fn chip_text(&self, label: &str, display: impl Fn(&str) -> String) -> String {
        const SHOWN: usize = 2;
        let mut values: Vec<String> = self.values.iter().map(|v| display(v)).collect();
        values.extend(self.custom.iter().map(|c| format!("contains “{c}”")));
        let extra = values.len().saturating_sub(SHOWN);
        values.truncate(SHOWN);
        let mut text = format!("{}{label}: {}", if self.hide { "not " } else { "" }, values.join(", "));
        if extra > 0 {
            text.push_str(&format!(" +{extra}"));
        }
        text
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
    /// The conditions are kept but not applied: the Focus Pane lists every
    /// line. Any change to a condition applies them again. Not saved.
    #[serde(skip)]
    pub paused: bool,
}

impl FilterState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        !self.time_range.is_active() && self.columns.iter().all(|c| !c.is_active())
    }

    /// Whether the Filter narrows the rows: it has a condition and is not paused.
    pub fn is_applied(&self) -> bool {
        !self.paused && !self.is_empty()
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
        // Changing a condition applies the Filter again.
        self.paused = false;
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
        self.paused = false;
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
        self.paused = false;
        self.columns.retain(|c| c.column != key);
    }

    pub fn set_time_range(&mut self, from: &str, to: &str) {
        self.paused = false;
        self.time_range = TimeRange { from: from.trim().to_string(), to: to.trim().to_string() };
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Show only (or with `hide`, hide) the rows whose `key` is `value`, from
    /// a selection: a Custom Value (contains) when `custom`, else a Filter
    /// Value. A condition of the same mode gets the value added; one of the
    /// other mode is replaced.
    pub fn apply_from_selection(&mut self, key: &str, value: &str, custom: bool, hide: bool) {
        let col = self.column_mut(key);
        if col.hide != hide {
            col.values.clear();
            col.custom.clear();
            col.hide = hide;
        }
        if custom {
            self.add_custom(key, value);
        } else {
            let folded = unicode_text::fold(value);
            if !self.is_value_ticked(key, &folded) {
                self.toggle_value(key, value);
            }
        }
    }

    /// The indices of the rows that pass the Filter.
    pub fn evaluate<R: Rows + ?Sized>(&self, index: &FilterIndex, rows: &R) -> Vec<usize> {
        self.evaluate_from(index, rows, 0)
    }

    /// The indices of the rows from `start` on that pass the Filter. Values
    /// come from `index`; `rows` reads the text of a value the index does
    /// not keep (a column with too many values, rows it does not cover).
    pub fn evaluate_from<R: Rows + ?Sized>(&self, index: &FilterIndex, rows: &R, start: usize) -> Vec<usize> {
        let (mut visible, text) = self.evaluate_split(index, rows.len(), start);
        if !text.is_empty() {
            visible.retain(|&row| text.iter().all(|c| c.passes_row(rows, row)));
        }
        visible
    }

    /// The Filter in two steps (task 5.3). The rows from `start` on (of
    /// `len`) that pass the conditions read from `index` alone (value ids,
    /// the Time Range), and the conditions that need the text of a row's
    /// value, for those rows to pass as well: a Scan reads it for lines
    /// read from disk.
    pub fn evaluate_split(&self, index: &FilterIndex, len: usize, start: usize) -> (Vec<usize>, Vec<TextCondition>) {
        if self.paused {
            return ((start..len).collect(), Vec::new());
        }
        let mut visible = Vec::with_capacity(len.saturating_sub(start));
        let stale = index.rows() != len;
        let mut by_ids: Vec<(&ColumnFilter, CompiledColumn)> = Vec::new();
        let mut text = Vec::new();
        for cf in self.columns.iter().filter(|c| c.is_active()) {
            match CompiledColumn::compile(cf, index.column(&cf.column), stale) {
                Some(compiled) => by_ids.push((cf, compiled)),
                None => text.push(TextCondition::new(cf)),
            }
        }
        let (from, to) = self.time_range.bounds(index.anchor_day());
        let time_active = (from.is_some() || to.is_some()) && !stale;

        for row in start..len {
            let columns_ok = by_ids.iter().all(|(cf, compiled)| compiled.matches(row) != cf.hide);
            let time_ok = !time_active
                || match index.effective_timestamp(row) {
                    None => true,
                    Some(ts) => from.is_none_or(|f| ts >= f) && to.is_none_or(|t| ts <= t),
                };
            if columns_ok && time_ok {
                visible.push(row);
            }
        }
        (visible, text)
    }
}

/// A column's condition that needs the text of a row's value: the Filter
/// has no value ids for the column (too many values, or rows it does not
/// cover).
#[derive(Debug, Clone)]
pub struct TextCondition {
    pub column: String,
    values: Vec<Needle>,
    custom: Vec<Needle>,
    hide: bool,
}

impl TextCondition {
    fn new(cf: &ColumnFilter) -> Self {
        Self {
            column: cf.column.clone(),
            values: cf.values.iter().map(|v| Needle::new(v)).collect(),
            custom: cf.custom.iter().map(|c| Needle::new(c)).collect(),
            hide: cf.hide,
        }
    }

    /// Whether a row whose value in the column is `value` passes.
    pub fn passes(&self, value: Option<&str>) -> bool {
        let matched =
            value.is_some_and(|v| self.values.iter().any(|n| n.equals(v)) || self.custom.iter().any(|n| n.is_in(v)));
        matched != self.hide
    }

    /// Whether the row `row` of `rows` passes.
    pub fn passes_row<R: Rows + ?Sized>(&self, rows: &R, row: usize) -> bool {
        with_value_of(rows, row, &self.column, |v| self.passes(v))
    }
}

/// A column's condition read from the column's value ids: a lookup table
/// over its (at most 500) values.
struct CompiledColumn<'a> {
    col: &'a ColumnIndex,
    table: Vec<bool>,
}

impl<'a> CompiledColumn<'a> {
    /// `None` when the column has no value ids for the rows: the condition
    /// then needs their text.
    fn compile(cf: &ColumnFilter, col: Option<&'a ColumnIndex>, stale: bool) -> Option<Self> {
        let col = col.filter(|c| !stale && c.has_row_ids())?;
        let custom: Vec<Needle> = cf.custom.iter().map(|c| Needle::new(c)).collect();
        let ticked: HashSet<&str> = cf.values.iter().map(String::as_str).collect();
        let table = col
            .values
            .iter()
            .map(|v| ticked.contains(v.folded.as_str()) || custom.iter().any(|n| n.is_in(&v.display)))
            .collect();
        Some(Self { col, table })
    }

    fn matches(&self, row: usize) -> bool {
        match self.col.id_of_row(row) {
            Some(MISSING) | None => false,
            Some(id) => self.table.get(id as usize).copied().unwrap_or(false),
        }
    }
}
