//! Logic: the columns of the Parsed View. Which columns show, their order and
//! Column Labels belong to the Project (its Columns section); the Field
//! Columns laid out are the ones turned on that have a value in the view.

use super::AppView;
use crate::column::PinnableColumn;
use crate::columns::{BUILT_IN, ColumnSettings, MESSAGE_KEY};
use crate::workspace::state::ContextSection;
use gpui::{AppContext as _, Context, Entity, Subscription, Window};
use gpui_component::input::{InputEvent, InputState};

/// How many Detected Fields the Columns section lists before the rest are
/// only found by searching.
pub const DETECTED_SHOWN: usize = 50;

/// A Column Label being edited in place on its Column Header.
pub struct LabelEdit {
    pub key: String,
    pub input: Entity<InputState>,
    /// Shown after Enter or leaving the field, hidden again on typing.
    pub error: Option<String>,
    /// The text the error is about.
    error_text: String,
    /// Edited in the Columns section rather than on the Column Header.
    pub in_panel: bool,
    /// Blur only commits once the input actually received focus.
    has_focused: bool,
    _subscription: Subscription,
}

/// One column in the Columns section.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnRow {
    pub key: String,
    /// Column Label or name.
    pub name: String,
    /// Parent Lines with a value in the view.
    pub lines: usize,
    pub shown: bool,
    /// MESSAGE has no switch.
    pub switchable: bool,
    /// Position among the Field Columns turned on, which are dragged to
    /// change the order they are drawn in.
    pub order: Option<usize>,
}

/// An Extraction Rule in the *Rules* group.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleRow {
    pub id: u64,
    /// Place in the group, which is the rule's priority.
    pub ix: usize,
    /// The field's Column Label or name.
    pub name: String,
    /// How the rule matches, in words.
    pub description: String,
    /// Parent Lines with a value in the field.
    pub lines: usize,
    pub enabled: bool,
}

/// The groups of the Columns section; an empty group is not shown.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ColumnGroups {
    pub built_in: Vec<ColumnRow>,
    /// Fields of the Log Formats the view uses, in template order.
    pub from_format: Vec<ColumnRow>,
    /// Extraction Rules in priority order.
    pub rules: Vec<RuleRow>,
    /// Detected Fields (JSON keys): the ones turned on first, in drawing
    /// order, then the others with the most lines first, at most
    /// [`DETECTED_SHOWN`] of them unless searched for.
    pub detected: Vec<ColumnRow>,
    /// Detected Fields left out of `detected`.
    pub detected_more: usize,
}

impl AppView {
    /// The Columns section of the active Project.
    pub fn column_settings(&self) -> ColumnSettings {
        self.active_project().map(|p| p.columns.clone()).unwrap_or_default()
    }

    /// Lay out the columns the Project shows: the fixed ones not hidden and
    /// the Field Columns turned on that have a value in the view.
    pub(crate) fn sync_view_columns(&mut self) {
        let settings = self.column_settings();
        self.formats.view_fields = self.format_fields_in_view();
        let fields = settings.view_fields_with(&self.filter_index, &self.formats.view_fields);
        self.column_layout.labels = settings.labels.clone();
        self.column_layout.set_view_columns(settings.hidden_columns(), fields, self.density);
        self.sync_pane_columns();
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

    /// The Field Columns the Focus Pane lays out: the ones on that have a
    /// value in a row. A Log Format's fields come first, then a rule's, then
    /// the fields the user ordered, then the rest.
    fn pane_field_keys(&self) -> Vec<crate::model::FieldKey> {
        let settings = self.column_settings();
        let mut keys: Vec<String> = self.formats.view_fields.clone();
        keys.extend(self.extraction_rules().iter().map(|r| r.name.clone()));
        keys.extend(settings.fields.iter().cloned());
        keys.extend(settings.pane.iter().filter(|(_, on)| **on).map(|(k, _)| k.clone()));
        let mut seen = std::collections::HashSet::new();
        keys.retain(|k| is_pane_field(k) && seen.insert(k.clone()));
        keys.into_iter()
            .filter(|k| self.column_shown(k) && self.filter_index.coverage(k).0 > 0)
            .map(|k| crate::model::FieldKey::new(&k))
            .collect()
    }

    /// Whether the Focus Pane shows the column `key` when the user made no
    /// choice: the fixed columns, a Log Format's and a rule's fields, and
    /// Source and Stream where the open Log Space has them.
    fn pane_default(&self, key: &str) -> bool {
        match key {
            "timestamp" | "level" | "module" | MESSAGE_KEY => true,
            "source" => self.active_space().is_some_and(|s| s.files.len() > 1),
            "stream" => self.active_space_has_command(),
            _ => {
                self.formats.view_fields.iter().any(|f| f == key)
                    || self.extraction_rules().iter().any(|r| r.name == key)
            }
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

    /// "Hide column" of a Column Header: the Canvas only.
    pub fn set_canvas_column_shown(&mut self, key: &str, shown: bool, cx: &mut Context<Self>) {
        match key {
            MESSAGE_KEY => {}
            "source" => self.set_show_source(shown, cx),
            "stream" => self.set_show_stream(shown, cx),
            _ => {
                let from_format = self.formats.view_fields.iter().any(|f| f == key);
                if let Some(project) = self.active_project_mut() {
                    if from_format {
                        project.columns.set_default_shown(key, shown);
                    } else {
                        project.columns.set_shown(key, shown);
                    }
                }
                self.columns_changed(cx);
                // A `key=value` field is read only while it is on.
                self.reread_if_config_changed(cx);
            }
        }
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

    /// Drag a Field Column from `from` to `to` in the Columns section.
    pub fn move_field_column(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        if let Some(project) = self.active_project_mut() {
            project.columns.move_field(from, to);
        }
        self.columns_changed(cx);
    }

    /// "Manage columns…": open the Columns section with the row of `key`
    /// picked out.
    pub fn manage_columns(&mut self, key: Option<&str>, cx: &mut Context<Self>) {
        self.context_sidebar.reveal(ContextSection::Columns);
        self.columns_highlighted = key.map(str::to_string);
        cx.notify();
    }

    /// Whether the Parsed View draws a Column Header for `key`.
    pub fn has_column_header(&self, key: &str) -> bool {
        PinnableColumn::from_key(key).is_none_or(|c| self.column_layout.columns().contains(&c))
    }

    /// The keys of every column of the view, for telling Column Labels apart.
    fn view_column_keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = BUILT_IN.iter().map(|(k, _)| k.to_string()).collect();
        for col in self.filter_index.field_columns() {
            keys.push(col.key.clone());
        }
        keys.extend(self.column_settings().fields);
        keys.sort();
        keys.dedup();
        keys
    }

    /// Start editing the Column Label of `key` on its Column Header.
    /// `in_panel` edits it in the Columns section instead, as it is for a
    /// column the Parsed View does not show.
    pub fn begin_column_rename(&mut self, key: &str, in_panel: bool, window: &mut Window, cx: &mut Context<Self>) {
        let in_panel = in_panel || !self.has_column_header(key);
        if in_panel {
            self.manage_columns(Some(key), cx);
        }
        let current = self.column_settings().display_name(key).to_string();
        let input = cx.new(|cx| InputState::new(window, cx).default_value(current));
        input.update(cx, |state, cx| {
            state.focus(window, cx);
            state.select_all(window, cx);
        });
        let subscription = cx.subscribe(&input, |this: &mut Self, _, event: &InputEvent, cx| match event {
            InputEvent::PressEnter { .. } => this.commit_column_label(cx),
            InputEvent::Change => {
                let Some(edit) = this.label_edit.as_ref() else { return };
                let typed = edit.input.read(cx).value().to_string() != edit.error_text;
                if let Some(edit) = this.label_edit.as_mut().filter(|e| typed && e.error.is_some()) {
                    edit.error = None;
                    cx.notify();
                }
            }
            InputEvent::Focus => {
                if let Some(edit) = this.label_edit.as_mut() {
                    edit.has_focused = true;
                }
            }
            InputEvent::Blur => {
                if this.label_edit.as_ref().is_some_and(|e| e.has_focused) {
                    this.commit_column_label(cx);
                }
            }
        });
        self.label_edit = Some(LabelEdit {
            key: key.to_string(),
            input,
            error: None,
            error_text: String::new(),
            in_panel,
            has_focused: false,
            _subscription: subscription,
        });
        cx.notify();
    }

    /// Keep the typed Column Label, or show why it cannot be kept.
    pub fn commit_column_label(&mut self, cx: &mut Context<Self>) {
        let Some(edit) = self.label_edit.as_ref() else { return };
        let key = edit.key.clone();
        let text = edit.input.read(cx).value().to_string();
        let keys = self.view_column_keys();
        let others: Vec<&str> = keys.iter().map(String::as_str).collect();
        let result = match self.active_project_mut() {
            Some(project) => project.columns.set_label(&key, &text, &others),
            None => Ok(()),
        };
        match result {
            Ok(()) => {
                self.label_edit = None;
                self.column_layout.labels = self.column_settings().labels;
                if let Some(col) = PinnableColumn::from_key(&key) {
                    let width = self.column_layout.width(col);
                    self.column_layout.auto_fit(col, self.density);
                    let fitted = self.column_layout.width(col);
                    self.column_layout.set_width(col, width.max(fitted));
                }
                self.columns_changed(cx);
            }
            Err(err) => {
                if let Some(edit) = self.label_edit.as_mut() {
                    edit.error = Some(err.message());
                    edit.error_text = text;
                }
                cx.notify();
            }
        }
    }

    pub fn cancel_column_label(&mut self, cx: &mut Context<Self>) {
        if self.label_edit.take().is_some() {
            cx.notify();
        }
    }

    /// What the column `key` is called on screen (Column Header, Filter,
    /// Highlights, Columns section).
    pub fn column_display_name(&self, key: &str) -> String {
        self.active_project()
            .map(|p| p.columns.display_name(key).to_string())
            .unwrap_or_else(|| crate::columns::default_name(key).to_string())
    }

    /// The rows of the Columns section, with Detected Fields narrowed to
    /// those whose name contains `query`.
    pub fn column_groups(&self, query: &str) -> ColumnGroups {
        let settings = self.column_settings();
        let index = &self.filter_index;
        let parents = index.parent_lines();
        let row = |key: &str, lines: usize, shown: bool, switchable: bool| ColumnRow {
            key: key.to_string(),
            name: settings.display_name(key).to_string(),
            lines,
            shown,
            switchable,
            order: None,
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
        let mut rule_names: Vec<&str> = self.extraction_rules().iter().map(|r| r.name.as_str()).collect();
        rule_names.extend(format_fields.iter().map(String::as_str));
        for (ix, r) in self.extraction_rules().iter().enumerate() {
            groups.rules.push(RuleRow {
                id: r.id,
                ix,
                name: settings.display_name(&r.name).to_string(),
                description: r.description.clone(),
                lines: if r.enabled { index.coverage(&r.name).0 } else { 0 },
                enabled: r.enabled,
            });
        }
        let query = query.trim().to_lowercase();
        for (i, key) in settings.fields.iter().enumerate() {
            if rule_names.contains(&key.as_str()) {
                continue;
            }
            groups.detected.push(ColumnRow { order: Some(i), ..row(key, index.coverage(key).0, self.column_shown(key), true) });
        }
        let mut detected: Vec<ColumnRow> = index
            .field_columns()
            .filter(|c| !settings.fields.contains(&c.key) && !rule_names.contains(&c.key.as_str()))
            .map(|c| row(&c.key, c.covered(), self.column_shown(&c.key), true))
            .filter(|r| query.is_empty() || r.key.to_lowercase().contains(&query) || r.name.to_lowercase().contains(&query))
            .collect();
        for (key, n) in index.kv_counts() {
            let key = key.as_str();
            let listed = settings.fields.iter().any(|f| f == key)
                || rule_names.contains(&key)
                || detected.iter().any(|r| r.key == key);
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

/// Whether `key` can be a Field Column of the Focus Pane: a field of the
/// lines, not one of the fixed columns.
fn is_pane_field(key: &str) -> bool {
    crate::model::is_field_name(key) && !BUILT_IN.iter().any(|(k, _)| *k == key)
}
