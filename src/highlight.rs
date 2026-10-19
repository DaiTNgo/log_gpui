//! Highlight Rules: a color for every row whose value in one column meets a
//! condition. A rule is written like a Filter condition (ticked values,
//! Custom Values containing text, joined by OR) but never hides a row.

use crate::filter_index::value_of;
use crate::model::LogEntry;
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

/// The paint of each row, shared by every row the same rule paints.
pub type RowPaints = Vec<Option<Arc<RowPaint>>>;

/// A rule with its conditions folded once, to test a whole log against it.
struct CompiledRule<'a> {
    column: &'a str,
    values: Vec<Needle>,
    custom: Vec<Needle>,
    paint: Arc<RowPaint>,
}

impl<'a> CompiledRule<'a> {
    fn new(index: usize, rule: &'a HighlightRule) -> Self {
        Self {
            column: &rule.column,
            values: rule.values.iter().map(|v| Needle::new(v)).collect(),
            custom: rule.custom.iter().map(|c| Needle::new(c)).collect(),
            paint: Arc::new(RowPaint { rule: index, color: rule.color.clone(), column: rule.column.clone() }),
        }
    }

    fn matches_value(&self, value: Option<&str>) -> bool {
        let Some(v) = value else { return false };
        self.values.iter().any(|n| n.equals(v)) || self.custom.iter().any(|n| n.is_in(v))
    }
}

fn compile(rules: &[HighlightRule]) -> Vec<CompiledRule<'_>> {
    rules
        .iter()
        .enumerate()
        .filter(|(_, r)| r.enabled)
        .map(|(i, r)| CompiledRule::new(i, r))
        .collect()
}

fn paint_compiled(rules: &[CompiledRule], entries: &[LogEntry], row: usize) -> Option<Arc<RowPaint>> {
    rules
        .iter()
        .find(|rule| rule.matches_value(value_of(entries, row, rule.column)))
        .map(|rule| rule.paint.clone())
}

/// The rule that paints `row`: the first enabled rule in the list that it
/// meets. A Continuation Line is judged by its Parent Line's values.
pub fn paint_for_row(rules: &[HighlightRule], entries: &[LogEntry], row: usize) -> Option<RowPaint> {
    paint_compiled(&compile(rules), entries, row).map(|p| (*p).clone())
}

/// The paint of every row of `entries`, for the rows' own order.
pub fn paint_rows(rules: &[HighlightRule], entries: &[LogEntry]) -> RowPaints {
    let compiled = compile(rules);
    if compiled.is_empty() {
        return vec![None; entries.len()];
    }
    crate::par::map_rows(entries.len(), |row| paint_compiled(&compiled, entries, row))
}

/// How many of `rows` (positions in `entries`) `rule` matches by its own
/// condition, for the count in the Highlights list. Continuation Lines are
/// not counted, like Filter counts.
pub fn count_matches(rule: &HighlightRule, entries: &[LogEntry], rows: &[usize]) -> usize {
    let compiled = CompiledRule::new(0, rule);
    crate::par::count(rows, |row| {
        entries.get(row).is_some_and(|e| e.parent.is_none())
            && compiled.matches_value(value_of(entries, row, &rule.column))
    })
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
