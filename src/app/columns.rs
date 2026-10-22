//! Logic: the columns of the Parsed View. Which columns the Focus Pane shows
//! belongs to the Project (its Columns section); the Field Columns laid out
//! are the ones turned on that have a value in the view. The order is fixed
//! by source: built-in, from Log Format, then Detected.

use super::AppView;
use crate::column::PinnableColumn;
use crate::columns::{BUILT_IN, ColumnSettings, MESSAGE_KEY};
use gpui::Context;

/// How many Detected Fields the Columns section lists before the rest are
/// only found by searching.
pub const DETECTED_SHOWN: usize = 50;

/// One column in the Columns section.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnRow {
    pub key: String,
    /// The column's name.
    pub name: String,
    /// Parent Lines with a value in the view.
    pub lines: usize,
    pub shown: bool,
    /// MESSAGE has no switch.
    pub switchable: bool,
}

/// The groups of the Columns section; an empty group is not shown.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ColumnGroups {
    pub built_in: Vec<ColumnRow>,
    /// Fields of the Log Formats the view uses, in template order.
    pub from_format: Vec<ColumnRow>,
    /// Detected Fields (JSON keys): the ones turned on first, then the others
    /// with the most lines first, at most [`DETECTED_SHOWN`] of them unless
    /// searched for.
    pub detected: Vec<ColumnRow>,
    /// Detected Fields left out of `detected`.
    pub detected_more: usize,
}

impl AppView {
    /// The Columns section of the active Project.
    pub fn column_settings(&self) -> ColumnSettings {
        self.active_project().map(|p| p.columns.clone()).unwrap_or_default()
    }

    /// Lay out the columns of the Canvas, which shows every column of the
    /// lines and the Log Space: the fixed ones, Source and Stream where the
    /// open view has them, and a Field Column for every Detected Field.
    pub(crate) fn sync_view_columns(&mut self) {
        self.formats.view_fields = self.format_fields_in_view();
        let fields = self.canvas_field_keys();
        self.column_layout.set_view_columns(Default::default(), fields, self.density);
        self.sync_pane_columns();
    }

    /// The Field Columns of a layout, in a fixed order by source: a Log
    /// Format's fields, then the Detected Fields in the order they were
    /// found, then `extra` (fields read only while on). Only fields with a
    /// value in a row that pass `keep`.
    fn field_keys(&self, extra: Vec<String>, keep: impl Fn(&str) -> bool) -> Vec<crate::model::FieldKey> {
        let mut keys: Vec<String> = self.formats.view_fields.clone();
        keys.extend(self.filter_index.field_columns().map(|c| c.key.clone()));
        keys.extend(extra);
        crate::columns::pane_fields(keys)
            .into_iter()
            .filter(|k| keep(k) && self.filter_index.coverage(k).0 > 0)
            .map(|k| crate::model::FieldKey::new(&k))
            .collect()
    }

    /// The Field Columns of the Canvas, which shows every one.
    fn canvas_field_keys(&self) -> Vec<crate::model::FieldKey> {
        self.field_keys(Vec::new(), |_| true)
    }

    /// Lay out the columns of the Focus Pane, which has its own switches in
    /// the Columns section.
    pub(crate) fn sync_pane_columns(&mut self) {
        use crate::column::PaneColumns;
        let mut pane = PaneColumns::default();
        for col in [PinnableColumn::Timestamp, PinnableColumn::Level, PinnableColumn::Module] {
            if !self.column_shown(col.key()) {
                pane.hidden.insert(col);
            }
        }
        pane.source = self.column_shown("source") && self.active_is_log_space();
        pane.stream = self.column_shown("stream") && self.active_space_has_command();
        pane.fields = self.pane_field_keys();
        self.column_layout.set_pane_columns(pane, self.density);
    }

    /// The Field Columns the Focus Pane lays out: the ones switched on (a
    /// `key=value` field read only while on comes last).
    fn pane_field_keys(&self) -> Vec<crate::model::FieldKey> {
        let on = self.column_settings().pane.into_iter().filter(|(_, on)| *on).map(|(k, _)| k).collect();
        self.field_keys(on, |k| self.column_shown(k))
    }

    /// Whether the Focus Pane shows the column `key` when the user made no
    /// choice: the fixed columns, a Log Format's fields, and Source and
    /// Stream where the open Log Space has them.
    fn pane_default(&self, key: &str) -> bool {
        match key {
            "timestamp" | "level" | "module" | MESSAGE_KEY => true,
            "source" => self.active_space().is_some_and(|s| s.files.len() > 1),
            "stream" => self.active_space_has_command(),
            _ => self.formats.view_fields.iter().any(|f| f == key),
        }
    }

    fn columns_changed(&mut self, cx: &mut Context<Self>) {
        self.sync_view_columns();
        self.schedule_session_save(cx);
        cx.notify();
    }

    /// The switch of a column in the Columns section: the Focus Pane only.
    pub fn set_column_shown(&mut self, key: &str, shown: bool, cx: &mut Context<Self>) {
        if key == MESSAGE_KEY {
            return;
        }
        if let Some(project) = self.active_project_mut() {
            project.columns.set_pane_shown(key, shown);
        }
        self.columns_changed(cx);
        // A `key=value` field is read only while it is on.
        self.reread_if_config_changed(cx);
    }

    /// Whether the Focus Pane shows the column `key` (its switch in the
    /// Columns section).
    pub fn column_shown(&self, key: &str) -> bool {
        if key == MESSAGE_KEY {
            return true;
        }
        self.active_project()
            .and_then(|p| p.columns.pane_choice(key))
            .unwrap_or_else(|| self.pane_default(key))
    }

    /// Whether the Parsed View draws a Column Header for `key`.
    pub fn has_column_header(&self, key: &str) -> bool {
        PinnableColumn::from_key(key).is_none_or(|c| self.column_layout.columns().contains(&c))
    }

    /// The rows of the Columns section, with Detected Fields narrowed to
    /// those whose name contains `query`.
    pub fn column_groups(&self, query: &str) -> ColumnGroups {
        let index = &self.filter_index;
        let parents = index.parent_lines();
        let row = |key: &str, lines: usize, shown: bool, switchable: bool| ColumnRow {
            key: key.to_string(),
            name: crate::columns::default_name(key).to_string(),
            lines,
            shown,
            switchable,
        };
        let mut groups = ColumnGroups::default();
        for (key, _) in BUILT_IN {
            let lines = match key {
                "timestamp" => index.timed_lines(),
                MESSAGE_KEY => parents,
                other => index.coverage(other).0,
            };
            let listed = match key {
                "source" => self.active_is_log_space(),
                "stream" => self.active_space_has_command(),
                _ => true,
            };
            if listed {
                groups.built_in.push(row(key, lines, self.column_shown(key), key != MESSAGE_KEY));
            }
        }
        for key in &self.formats.view_fields {
            groups.from_format.push(row(key, index.coverage(key).0, self.column_shown(key), true));
        }
        let format_fields = &self.formats.view_fields;
        let query = query.trim().to_lowercase();
        let mut detected: Vec<ColumnRow> = index
            .field_columns()
            .filter(|c| !format_fields.contains(&c.key))
            .map(|c| row(&c.key, c.covered(), self.column_shown(&c.key), true))
            .filter(|r| query.is_empty() || r.key.to_lowercase().contains(&query) || r.name.to_lowercase().contains(&query))
            .collect();
        for (key, n) in index.kv_counts() {
            let key = key.as_str();
            let listed = format_fields.iter().any(|f| f == key) || detected.iter().any(|r| r.key == key);
            if !listed && (query.is_empty() || key.to_lowercase().contains(&query)) {
                detected.push(row(key, *n, self.column_shown(key), true));
            }
        }
        detected.sort_by(|a, b| b.shown.cmp(&a.shown).then(b.lines.cmp(&a.lines)).then_with(|| a.key.cmp(&b.key)));
        groups.detected_more = detected.len().saturating_sub(DETECTED_SHOWN);
        detected.truncate(DETECTED_SHOWN);
        groups.detected.extend(detected);
        groups
    }
}
