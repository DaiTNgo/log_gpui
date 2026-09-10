use crate::model::{LogEntry, LogLevel};
use crate::query::ast::{GroupOperator, QueryExpression};
use crate::query::operator::QueryOperator;
use crate::query::value::QueryValue;
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilterType {
    Include,
    Exclude,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConditionField {
    Level,
    Module,
    Message,
    RequestId,
}

impl ConditionField {
    pub fn as_str(&self) -> &'static str {
        match self {
            ConditionField::Level => "Level",
            ConditionField::Module => "Module",
            ConditionField::Message => "Message",
            ConditionField::RequestId => "Request ID",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConditionOp {
    Is,
    IsNot,
    Contains,
    NotContains,
    MatchesRegex,
}

impl ConditionOp {
    #[allow(non_upper_case_globals)]
    pub const Equals: ConditionOp = ConditionOp::Is;
    #[allow(non_upper_case_globals)]
    pub const NotEquals: ConditionOp = ConditionOp::IsNot;

    pub fn as_str(&self) -> &'static str {
        match self {
            ConditionOp::Is => "is",
            ConditionOp::IsNot => "is not",
            ConditionOp::Contains => "contains",
            ConditionOp::NotContains => "doesn't contain",
            ConditionOp::MatchesRegex => "regex",
        }
    }
}

/// A filter row shown in the Filter Popover UI. Evaluation is delegated to the
/// shared `QueryEngine` via `FilterEngine::to_query_expression`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterRow {
    pub id: usize,
    pub field: ConditionField,
    pub op: ConditionOp,
    pub value: String,
    pub enabled: bool,
}

impl FilterRow {
    pub fn new(id: usize, field: ConditionField, op: ConditionOp, value: String) -> Self {
        Self {
            id,
            field,
            op,
            value,
            enabled: true,
        }
    }

    pub fn to_chip_label(&self) -> String {
        match self.field {
            ConditionField::Level => self.value.to_uppercase(),
            ConditionField::Module => self.value.clone(),
            ConditionField::Message => {
                if self.op == ConditionOp::IsNot || self.op == ConditionOp::NotContains {
                    format!("!{}", self.value)
                } else {
                    self.value.clone()
                }
            }
            ConditionField::RequestId => format!("req:{}", self.value),
        }
    }

    /// Map this row onto a query-language condition. Returns `None` for
    /// operators without an AST equivalent (forcing the legacy path).
    fn to_query_condition(&self) -> Option<(String, QueryOperator, QueryValue)> {
        let field = match self.field {
            ConditionField::Level => "level",
            ConditionField::Module => "module",
            ConditionField::Message => "message",
            ConditionField::RequestId => "requestId",
        };
        let op = match self.op {
            ConditionOp::Is => QueryOperator::Eq,
            ConditionOp::IsNot => QueryOperator::Neq,
            ConditionOp::Contains => QueryOperator::Contains,
            ConditionOp::MatchesRegex => QueryOperator::Matches,
            ConditionOp::NotContains => return None,
        };
        Some((field.to_string(), op, QueryValue::String(self.value.clone())))
    }

    pub fn matches(&self, entry: &LogEntry) -> bool {
        if !self.enabled || self.value.trim().is_empty() {
            return true;
        }

        let actual_val: &str = match self.field {
            ConditionField::Level => entry.level.as_str(),
            ConditionField::Module => &entry.tag,
            ConditionField::Message => &entry.payload,
            ConditionField::RequestId => entry.request_id.as_deref().unwrap_or(""),
        };

        match self.op {
            ConditionOp::Is => actual_val.eq_ignore_ascii_case(&self.value),
            ConditionOp::IsNot => !actual_val.eq_ignore_ascii_case(&self.value),
            ConditionOp::Contains => actual_val
                .to_lowercase()
                .contains(&self.value.to_lowercase()),
            ConditionOp::NotContains => !actual_val
                .to_lowercase()
                .contains(&self.value.to_lowercase()),
            ConditionOp::MatchesRegex => {
                if let Ok(re) = Regex::new(&self.value) {
                    re.is_match(actual_val)
                } else {
                    false
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogicMode {
    All, // AND
    Any, // OR
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterEngine {
    pub conditions: Vec<FilterRow>,
    pub logic_mode: LogicMode,
}

impl Default for FilterEngine {
    fn default() -> Self {
        Self {
            conditions: Vec::new(),
            logic_mode: LogicMode::All,
        }
    }
}

impl FilterEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.conditions.iter().all(|c| !c.enabled || c.value.is_empty())
    }

    pub fn add_condition(&mut self, field: ConditionField, op: ConditionOp, value: String) -> usize {
        let id = next_filter_id();
        self.conditions.push(FilterRow::new(id, field, op, value));
        id
    }

    pub fn remove_condition(&mut self, id: usize) {
        self.conditions.retain(|c| c.id != id);
    }

    pub fn clear(&mut self) {
        self.conditions.clear();
    }

    /// Add a quick filter from log row click
    pub fn add_quick_level(&mut self, level: LogLevel) {
        // Replace or add
        self.conditions.retain(|c| c.field != ConditionField::Level);
        self.add_condition(ConditionField::Level, ConditionOp::Is, level.as_str().to_string());
    }

    pub fn hide_level(&mut self, level: LogLevel) {
        self.add_condition(ConditionField::Level, ConditionOp::IsNot, level.as_str().to_string());
    }

    pub fn add_quick_module(&mut self, module: &str) {
        self.conditions.retain(|c| c.field != ConditionField::Module);
        self.add_condition(ConditionField::Module, ConditionOp::Is, module.to_string());
    }

    pub fn hide_module(&mut self, module: &str) {
        self.add_condition(ConditionField::Module, ConditionOp::IsNot, module.to_string());
    }

    /// Build a query-language expression from the enabled, non-empty rows.
    /// Returns `None` when rows cannot map to the AST (e.g. `NotContains`).
    pub fn to_query_expression(&self) -> Option<QueryExpression> {
        let active_conditions: Vec<&FilterRow> = self
            .conditions
            .iter()
            .filter(|c| c.enabled && !c.value.trim().is_empty())
            .collect();
        if active_conditions.is_empty() {
            return None;
        }

        let mut children = Vec::with_capacity(active_conditions.len());
        for cond in active_conditions {
            let (field, op, value) = cond.to_query_condition()?;
            children.push(QueryExpression::Condition(crate::query::ast::QueryCondition::new(
                &field, op, value,
            )));
        }

        let operator = match self.logic_mode {
            LogicMode::All => GroupOperator::And,
            LogicMode::Any => GroupOperator::Or,
        };
        Some(QueryExpression::group(operator, children))
    }

    /// Evaluate active filters on all entries and return visible indices.
    /// Delegates to the shared `QueryEngine` when possible; falls back to the
    /// per-row matcher for rows without an AST equivalent.
    pub fn evaluate(&self, entries: &mut [LogEntry]) -> Vec<usize> {
        let active_conditions: Vec<&FilterRow> = self
            .conditions
            .iter()
            .filter(|c| c.enabled && !c.value.trim().is_empty())
            .collect();

        if active_conditions.is_empty() {
            let mut prev_ts: Option<f64> = None;
            for entry in entries.iter_mut() {
                update_entry_delta_time(entry, &mut prev_ts);
            }
            return (0..entries.len()).collect();
        }

        let mut visible = Vec::with_capacity(entries.len());
        let mut prev_ts: Option<f64> = None;

        if let Some(expr) = self.to_query_expression() {
            for (idx, entry) in entries.iter_mut().enumerate() {
                if crate::query::engine::QueryEngine::evaluate_entry(entry, &expr) {
                    update_entry_delta_time(entry, &mut prev_ts);
                    visible.push(idx);
                }
            }
            return visible;
        }

        for (idx, entry) in entries.iter_mut().enumerate() {
            let passes = match self.logic_mode {
                LogicMode::All => active_conditions.iter().all(|c| c.matches(entry)),
                LogicMode::Any => active_conditions.iter().any(|c| c.matches(entry)),
            };

            if passes {
                update_entry_delta_time(entry, &mut prev_ts);
                visible.push(idx);
            }
        }

        visible
    }
}

fn update_entry_delta_time(entry: &mut LogEntry, prev_ts: &mut Option<f64>) {
    if let Some(prev) = *prev_ts {
        entry.delta_time_ms = Some((entry.timestamp - prev) * 1000.0);
    } else {
        entry.delta_time_ms = Some(0.0);
    }
    *prev_ts = Some(entry.timestamp);
}

static NEXT_FILTER_ID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);

fn next_filter_id() -> usize {
    NEXT_FILTER_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}
