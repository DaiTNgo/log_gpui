//! The Columns section of a Project: which columns the Focus Pane shows.
//! Columns are named by their column key (`level`, `source`, a field name),
//! the same key Filter and Highlight Rules use. The order is not stored: the
//! Focus Pane lays them out by source (built-in, from Log Format, Detected).
//! Column Labels and Field orders saved by older versions are ignored when
//! read and dropped on the next save.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Column key of MESSAGE, which is always shown.
pub const MESSAGE_KEY: &str = "message";

/// The fixed columns the Columns section lists in its *Built-in* group, by
/// key, with the name their Column Header shows.
pub const BUILT_IN: [(&str, &str); 6] = [
    ("timestamp", "TIMESTAMP"),
    ("level", "LEVEL"),
    ("module", "MODULE"),
    ("source", "SOURCE"),
    ("stream", "STREAM"),
    (MESSAGE_KEY, "MESSAGE"),
];

/// The keys that can be Field Columns of the Focus Pane, from `keys`: the
/// fields of the lines, not the fixed columns, each once, in first-seen order.
pub fn pane_fields(keys: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    keys.into_iter()
        .filter(|k| crate::model::is_field_name(k) && !BUILT_IN.iter().any(|(b, _)| b == k) && seen.insert(k.clone()))
        .collect()
}

/// The name a column shows.
pub fn default_name(key: &str) -> &str {
    BUILT_IN.iter().find(|(k, _)| *k == key).map_or(key, |(_, name)| name)
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnSettings {
    /// What the user chose for the Focus Pane in the Columns section, by
    /// column key. A column without a choice follows its default: the fixed
    /// columns and a Log Format's fields are on, Detected Fields are off.
    #[serde(default)]
    pub pane: BTreeMap<String, bool>,
}

impl ColumnSettings {
    /// The choice for the Focus Pane about the column `key`, if any.
    pub fn pane_choice(&self, key: &str) -> Option<bool> {
        self.pane.get(key).copied()
    }

    /// Show or hide the column `key` in the Focus Pane. MESSAGE is always
    /// shown.
    pub fn set_pane_shown(&mut self, key: &str, shown: bool) {
        if key != MESSAGE_KEY {
            self.pane.insert(key.to_string(), shown);
        }
    }
}

/// Coverage as the Column Header shows it: a percentage rounded down, or
/// nothing when every Parent Line has a value.
pub fn coverage_label(covered: usize, total: usize) -> Option<String> {
    if total == 0 || covered >= total {
        return None;
    }
    Some(format!("{}%", covered * 100 / total))
}

/// The tooltip of Coverage: "1,204 of 1,530 lines".
pub fn coverage_tooltip(covered: usize, total: usize) -> String {
    format!("{} of {} lines", crate::app::format_count(covered), crate::app::format_count(total))
}
