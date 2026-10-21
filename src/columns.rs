//! The Columns section of a Project: which columns the Parsed View shows, in
//! which order, and the Column Label of each. Columns are named by their
//! column key (`level`, `source`, a field name), the same key Filter and
//! Highlight Rules use, so a Column Label never changes what they match.

use crate::filter_index::FilterIndex;
use crate::model::{FieldKey, is_field_name};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};

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

/// The name a column shows when it has no Column Label.
pub fn default_name(key: &str) -> &str {
    BUILT_IN.iter().find(|(k, _)| *k == key).map_or(key, |(_, name)| name)
}

/// Why a Column Label was not kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LabelError {
    /// Another column of the view is already called this.
    Taken(String),
}

impl LabelError {
    /// What to tell the user, with how to fix it.
    pub fn message(&self) -> String {
        match self {
            LabelError::Taken(name) => format!("\"{name}\" is already a column here — pick another name"),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnSettings {
    /// Column Labels by column key.
    #[serde(default)]
    pub labels: HashMap<String, String>,
    /// Fixed columns the user hid (`timestamp`, `level`, `module`).
    #[serde(default)]
    pub hidden: BTreeSet<String>,
    /// Field Columns turned on, in the order the Parsed View draws them.
    #[serde(default)]
    pub fields: Vec<String>,
    /// What the user chose for the Focus Pane in the Columns section, by
    /// column key. A column without a choice follows its default: the fixed
    /// columns, a Log Format's and an Extraction Rule's fields are on,
    /// Detected Fields are off.
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

    /// The Column Label of `key`, if the user gave one.
    pub fn label(&self, key: &str) -> Option<&str> {
        self.labels.get(key).map(String::as_str)
    }

    /// What the column `key` is called on screen: its Column Label or name.
    pub fn display_name<'a>(&'a self, key: &'a str) -> &'a str {
        self.label(key).unwrap_or_else(|| default_name(key))
    }

    /// Whether the column `key` is shown (the Source and Stream Columns have
    /// switches of their own).
    pub fn is_shown(&self, key: &str) -> bool {
        if is_field_name(key) && !matches!(key, "source" | "stream") {
            self.fields.iter().any(|f| f == key)
        } else {
            !self.hidden.contains(key)
        }
    }

    /// Show or hide the column `key`. MESSAGE is always shown; a Field
    /// Column turned on goes to the end.
    pub fn set_shown(&mut self, key: &str, shown: bool) {
        if key == MESSAGE_KEY {
            return;
        }
        if is_field_name(key) && !matches!(key, "source" | "stream") {
            let at = self.fields.iter().position(|f| f == key);
            match (shown, at) {
                (true, None) => self.fields.push(key.to_string()),
                (false, Some(i)) => {
                    self.fields.remove(i);
                }
                _ => {}
            }
        } else if shown {
            self.hidden.remove(key);
        } else {
            self.hidden.insert(key.to_string());
        }
    }

    /// Show or hide a field shown unless hidden: a field of the Log Format in
    /// use.
    pub fn set_default_shown(&mut self, key: &str, shown: bool) {
        if shown {
            self.hidden.remove(key);
        } else {
            self.hidden.insert(key.to_string());
            self.fields.retain(|f| f != key);
        }
    }

    /// The Field Columns the view lays out: the ones turned on that have a
    /// value in a row, then the `default_on` fields (of the Log Format in
    /// use) not hidden, in their order.
    pub fn view_fields_with(&self, index: &FilterIndex, default_on: &[String]) -> Vec<FieldKey> {
        let mut out = self.view_fields(index);
        for f in default_on {
            let key = FieldKey::new(f);
            if !self.hidden.contains(f) && !out.contains(&key) && index.coverage(f).0 > 0 {
                out.push(key);
            }
        }
        out
    }

    /// Move the Field Column at `from` to `to` in the drawing order.
    pub fn move_field(&mut self, from: usize, to: usize) {
        if from >= self.fields.len() || from == to {
            return;
        }
        let f = self.fields.remove(from);
        self.fields.insert(to.min(self.fields.len()), f);
    }

    /// Fixed columns hidden, as Parsed View columns.
    pub fn hidden_columns(&self) -> BTreeSet<crate::column::PinnableColumn> {
        use crate::column::PinnableColumn;
        self.hidden
            .iter()
            .filter_map(|k| PinnableColumn::from_key(k))
            .filter(|c| matches!(c, PinnableColumn::Timestamp | PinnableColumn::Level | PinnableColumn::Module))
            .collect()
    }

    /// The Field Columns the view lays out: the ones turned on that have a
    /// value in at least one row, in order.
    pub fn view_fields(&self, index: &FilterIndex) -> Vec<FieldKey> {
        self.fields
            .iter()
            .filter(|f| index.coverage(f).0 > 0)
            .map(|f| FieldKey::new(f))
            .collect()
    }

    /// Give the column `key` the Column Label `text`; blank gives it back
    /// its name. `others` are the keys of the other columns of the view: a
    /// label may not be any of their names or labels.
    pub fn set_label(&mut self, key: &str, text: &str, others: &[&str]) -> Result<(), LabelError> {
        let text = text.trim();
        if text.is_empty() || text == default_name(key) {
            self.labels.remove(key);
            return Ok(());
        }
        let taken = others.iter().filter(|k| **k != key).any(|k| {
            let name = self.display_name(k);
            name.eq_ignore_ascii_case(text) || default_name(k).eq_ignore_ascii_case(text)
        });
        if taken {
            return Err(LabelError::Taken(text.to_string()));
        }
        self.labels.insert(key.to_string(), text.to_string());
        Ok(())
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
