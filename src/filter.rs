use crate::model::{LogEntry, LogLevel};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterCondition {
    pub id: usize,
    pub field: ConditionField,
    pub op: ConditionOp,
    pub value: String,
    pub enabled: bool,
}

impl FilterCondition {
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
    pub conditions: Vec<FilterCondition>,
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
        self.conditions.push(FilterCondition::new(id, field, op, value));
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

    /// Evaluate active filters on all entries and return visible indices
    pub fn evaluate(&self, entries: &mut [LogEntry]) -> Vec<usize> {
        let active_conditions: Vec<&FilterCondition> = self
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
