//! Highlight Rules: a color for every row whose value in one column meets a
//! condition. A rule is written like a Filter condition (ticked values,
//! Custom Values containing text, joined by OR) but never hides a row.

use crate::rows::Rows;
use crate::filter_index::{ColumnIndex, FilterIndex, MISSING, with_value_of};
use crate::palette::HighlightColor;
use crate::unicode_text::{self, Needle};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Column key of the message text. Highlight Rules may use it, with Custom
/// Values only; it is not a Filter column.
pub const MESSAGE_COLUMN: &str = "message";
/// Column key of the timestamp, which Highlight Rules cannot use.
pub const TIMESTAMP_COLUMN: &str = "timestamp";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HighlightRule {
    pub id: u64,
    /// Column key (`level`, `module`, `source`, `message`, a JSON field …).
    pub column: String,
    /// Ticked Filter Values, in folded form.
    #[serde(default)]
    pub values: Vec<String>,
    /// Custom Values: rows whose value contains the text.
    #[serde(default)]
    pub custom: Vec<String>,
    pub color: HighlightColor,
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
}

fn enabled_by_default() -> bool {
    true
}

/// Why a Highlight Rule could not be made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleError {
    /// TIMESTAMP is not a column of Highlight Rules.
    TimestampColumn,
    /// MESSAGE takes Custom Values only.
    MessageNeedsCustomValue,
    /// No value and no Custom Value.
    NoCondition,
}

impl HighlightRule {
    /// Make a rule. Values are given in any case and stored folded; blank
    /// Custom Values are dropped.
    pub fn new(
        id: u64,
        column: &str,
        values: &[String],
        custom: &[String],
        color: HighlightColor,
    ) -> Result<Self, RuleError> {
        if column == TIMESTAMP_COLUMN {
            return Err(RuleError::TimestampColumn);
        }
        let mut folded: Vec<String> = Vec::new();
        for v in values {
            let f = unicode_text::fold(v);
            if !folded.contains(&f) {
                folded.push(f);
            }
        }
        let mut customs: Vec<String> = Vec::new();
        for c in custom {
            let c = c.trim();
            if !c.is_empty() && !customs.iter().any(|x| unicode_text::fold(x) == unicode_text::fold(c)) {
                customs.push(c.to_string());
            }
        }
        if column == MESSAGE_COLUMN && !folded.is_empty() {
            return Err(RuleError::MessageNeedsCustomValue);
        }
        if folded.is_empty() && customs.is_empty() {
            return Err(RuleError::NoCondition);
        }
        Ok(Self { id, column: column.to_string(), values: folded, custom: customs, color, enabled: true })
    }

    /// Whether a row holding `value` in this rule's column meets a condition.
    /// A row with no value never does.
    pub fn matches_value(&self, value: Option<&str>) -> bool {
        let Some(v) = value else { return false };
        self.values.iter().any(|f| Needle::new(f).equals(v))
            || self.custom.iter().any(|c| Needle::new(c).is_in(v))
    }

    /// The conditions as shown in the list: `DB, CACHE, "timeout"`.
    pub fn summary(&self) -> String {
        let mut parts: Vec<String> = self.values.clone();
        parts.extend(self.custom.iter().map(|c| format!("\"{c}\"")));
        parts.join(", ")
    }
}

/// The paint of one row: the rule that won and whether it is the winning
/// rule's cell condition that this row meets (always, by construction: a rule
/// has one column, so the cell is that column's).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowPaint {
    /// Position of the winning rule in the list.
    pub rule: usize,
    pub color: HighlightColor,
    /// Column key of the cell that meets the condition.
    pub column: String,
}

/// Most rules that paint: a row keeps its rule in one byte.
pub const MAX_PAINTING_RULES: usize = u8::MAX as usize;

/// The paint of each row: one byte per row, the rule that won (0: none),
/// and the paints of the rules (task 7.1).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RowPaints {
    paints: Vec<Arc<RowPaint>>,
    rows: Vec<u8>,
}

impl RowPaints {
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The paint of `row`, if a rule paints it.
    pub fn get(&self, row: usize) -> Option<&RowPaint> {
        let at = *self.rows.get(row)?;
        at.checked_sub(1).and_then(|i| self.paints.get(i as usize)).map(|p| &**p)
    }

    /// The paint of each row in order.
    pub fn iter(&self) -> impl Iterator<Item = Option<&RowPaint>> + '_ {
        (0..self.rows.len()).map(|row| self.get(row))
    }

    /// Add the paints of the rows after these, painted by the same rules.
    pub fn extend(&mut self, more: RowPaints) {
        if !more.paints.is_empty() {
            self.paints = more.paints;
        }
        self.rows.extend(more.rows);
    }

    /// The byte of each row, to drop or move rows with.
    pub fn rows_mut(&mut self) -> &mut Vec<u8> {
        &mut self.rows
    }
}


/// A rule with its conditions folded once, to test a whole log against it.
struct CompiledRule<'a> {
    column: &'a str,
    values: Vec<Needle>,
    custom: Vec<Needle>,
    paint: Arc<RowPaint>,
    /// With the column's value ids: whether each value meets the rule, so a
    /// row is tested by its id instead of its text.
    ids: Option<(&'a ColumnIndex, Vec<bool>)>,
}

impl<'a> CompiledRule<'a> {
    fn new(index: usize, rule: &'a HighlightRule, columns: Option<&'a FilterIndex>) -> Self {
        let mut compiled = Self {
            column: &rule.column,
            values: rule.values.iter().map(|v| Needle::new(v)).collect(),
            custom: rule.custom.iter().map(|c| Needle::new(c)).collect(),
            paint: Arc::new(RowPaint { rule: index, color: rule.color.clone(), column: rule.column.clone() }),
            ids: None,
        };
        if let Some(col) = columns.and_then(|c| c.column(&rule.column)).filter(|c| c.has_row_ids()) {
            let ticked: Vec<&str> = rule.values.iter().map(String::as_str).collect();
            let table = col
                .values
                .iter()
                .map(|v| ticked.contains(&v.folded.as_str()) || compiled.custom.iter().any(|n| n.is_in(&v.display)))
                .collect();
            compiled.ids = Some((col, table));
        }
        compiled
    }

    fn matches_value(&self, value: Option<&str>) -> bool {
        let Some(v) = value else { return false };
        self.values.iter().any(|n| n.equals(v)) || self.custom.iter().any(|n| n.is_in(v))
    }

    /// Whether `row` meets the rule: by its value id when the column keeps
    /// them, else by its text.
    fn matches_row<R: Rows + ?Sized>(&self, entries: &R, row: usize) -> bool {
        if let Some((col, table)) = &self.ids {
            if let Some(id) = col.id_of_row(row) {
                return id != MISSING && table.get(id as usize).copied().unwrap_or(false);
            }
        }
        with_value_of(entries, row, self.column, |v| self.matches_value(v))
    }
}

fn compile<'a>(rules: &'a [HighlightRule], columns: Option<&'a FilterIndex>) -> Vec<CompiledRule<'a>> {
    rules
        .iter()
        .enumerate()
        .filter(|(_, r)| r.enabled)
        .take(MAX_PAINTING_RULES)
        .map(|(i, r)| CompiledRule::new(i, r, columns))
        .collect()
}

fn paint_compiled<R: Rows + ?Sized>(rules: &[CompiledRule], entries: &R, row: usize) -> Option<Arc<RowPaint>> {
    rules.iter().find(|rule| rule.matches_row(entries, row)).map(|rule| rule.paint.clone())
}

/// The rule that paints `row`: the first enabled rule in the list that it
/// meets. A Continuation Line is judged by its Parent Line's values.
pub fn paint_for_row<R: Rows + ?Sized>(rules: &[HighlightRule], entries: &R, row: usize) -> Option<RowPaint> {
    paint_compiled(&compile(rules, None), entries, row).map(|p| (*p).clone())
}

/// The paint of every row of `entries`, for the rows' own order.
pub fn paint_rows<R: Rows + ?Sized>(rules: &[HighlightRule], entries: &R) -> RowPaints {
    paint_rows_from(rules, entries, 0)
}

/// The paint of the rows from `from` on, to go after the paints of the rows
/// before it.
pub fn paint_rows_from<R: Rows + ?Sized>(rules: &[HighlightRule], entries: &R, from: usize) -> RowPaints {
    paint_rows_with(rules, entries, None, from)
}

/// `paint_rows_from`, testing rows by the value ids of `columns` (the
/// rows' Filter columns) where it keeps them, without reading the rows.
pub fn paint_rows_with<R: Rows + ?Sized>(
    rules: &[HighlightRule],
    entries: &R,
    columns: Option<&FilterIndex>,
    from: usize,
) -> RowPaints {
    // Value ids of rows other than these say nothing of them.
    let columns = columns.filter(|c| c.rows() == entries.len());
    let compiled = compile(rules, columns);
    let count = entries.len().saturating_sub(from);
    let paints = compiled.iter().map(|rule| rule.paint.clone()).collect();
    if compiled.is_empty() {
        return RowPaints { paints, rows: vec![0; count] };
    }
    let rows = crate::par::map_rows(count, |i| {
        compiled.iter().position(|rule| rule.matches_row(entries, from + i)).map_or(0, |at| at as u8 + 1)
    });
    RowPaints { paints, rows }
}

/// A rule whose column has no value ids: telling whether it meets a row
/// takes the text of the row's value (task 7.2).
#[derive(Debug, Clone)]
pub struct TextRule {
    pub id: u64,
    /// The byte of the rows it paints (its place among the rules, from 1).
    pub at: u8,
    pub column: String,
    values: Vec<Needle>,
    custom: Vec<Needle>,
}

impl TextRule {
    pub fn matches(&self, value: Option<&str>) -> bool {
        let Some(v) = value else { return false };
        self.values.iter().any(|n| n.equals(v)) || self.custom.iter().any(|n| n.is_in(v))
    }
}

/// The paints of a view whose rows are not all read: the rows `read` says
/// are painted by every rule; the others by the rules told by value ids,
/// unless a rule that needs the text comes first (then they wait, at 0).
pub struct SplitPaints {
    pub paints: RowPaints,
    /// The rules that need the text, in order.
    pub text: Vec<TextRule>,
    /// The rows not read, ascending, with the byte the rules told by value
    /// ids give them: a Scan reads them and checks `text` (empty when
    /// `text` is).
    pub unread: Vec<(usize, u8)>,
}

/// `paint_rows_with` for the rows from `from` on, reading only the rows
/// `read` says (the rows on screen, the rows held in memory).
pub fn paint_rows_split<R: Rows + ?Sized>(
    rules: &[HighlightRule],
    entries: &R,
    columns: Option<&FilterIndex>,
    from: usize,
    read: impl Fn(usize) -> bool + Sync,
) -> SplitPaints {
    let columns = columns.filter(|c| c.rows() == entries.len());
    let compiled = compile(rules, columns);
    let text: Vec<TextRule> = compiled
        .iter()
        .zip(rules.iter().filter(|r| r.enabled))
        .enumerate()
        .filter(|(_, (c, _))| c.ids.is_none())
        .map(|(at, (c, rule))| TextRule {
            id: rule.id,
            at: at as u8 + 1,
            column: rule.column.clone(),
            values: c.values.clone(),
            custom: c.custom.clone(),
        })
        .collect();
    let Some(first_text) = text.first().map(|t| t.at) else {
        return SplitPaints { paints: paint_rows_with(rules, entries, columns, from), text, unread: Vec::new() };
    };
    let count = entries.len().saturating_sub(from);
    let paints: Vec<Arc<RowPaint>> = compiled.iter().map(|rule| rule.paint.clone()).collect();
    let each: Vec<(u8, bool)> = crate::par::map_rows(count, |i| {
        let row = from + i;
        if read(row) {
            let won = compiled.iter().position(|rule| rule.matches_row(entries, row));
            return (won.map_or(0, |at| at as u8 + 1), false);
        }
        let by_ids = compiled.iter().position(|rule| rule.ids.is_some() && rule.matches_row(entries, row));
        (by_ids.map_or(0, |at| at as u8 + 1), true)
    });
    let mut unread = Vec::new();
    let rows = each
        .into_iter()
        .enumerate()
        .map(|(i, (byte, waits))| {
            if !waits {
                return byte;
            }
            unread.push((from + i, byte));
            if byte != 0 && byte < first_text { byte } else { 0 }
        })
        .collect();
    SplitPaints { paints: RowPaints { paints, rows }, text, unread }
}

/// How many of `rows` (positions in `entries`) `rule` matches by its own
/// condition, for the count in the Highlights list. Continuation Lines are
/// not counted, like Filter counts.
pub fn count_matches<R: Rows + ?Sized>(rule: &HighlightRule, entries: &R, rows: &[usize]) -> usize {
    count_matches_with(rule, entries, None, rows)
}

/// `count_matches` by the value ids of `columns` where it keeps them.
pub fn count_matches_with<R: Rows + ?Sized>(
    rule: &HighlightRule,
    entries: &R,
    columns: Option<&FilterIndex>,
    rows: &[usize],
) -> usize {
    let columns = columns.filter(|c| c.rows() == entries.len());
    let compiled = CompiledRule::new(0, rule, columns);
    crate::par::count(rows, |row| !entries.continues(row) && compiled.matches_row(entries, row))
}

/// The next unused rule id of a list.
pub fn next_rule_id(rules: &[HighlightRule]) -> u64 {
    rules.iter().map(|r| r.id).max().map_or(1, |m| m + 1)
}

/// Move the rule at `from` so it ends up at `to` (priority order).
pub fn move_rule(rules: &mut Vec<HighlightRule>, from: usize, to: usize) {
    if from >= rules.len() || to >= rules.len() || from == to {
        return;
    }
    let rule = rules.remove(from);
    rules.insert(to, rule);
}
