//! Logic: the New Column dialog. "Make column…" on a selected stretch of a
//! line opens it with the ways the app guessed to read that stretch from
//! every line; the user picks one (or edits its regex), names the column and
//! sees it on real lines before adding the Extraction Rule. "Edit" on a rule
//! opens it on the rule's regex.

use super::AppView;
use crate::extract::{
    Candidate, NameCheck, PatternError, Preview, RuleScope, SAMPLE_LINES, check_name, compile_pattern, guess,
    preview, sample_rows, suggest_name,
};
use crate::pattern_field::{PatternField, SETTLE};
use gpui::{AppContext as _, Context, Entity, Subscription, Window};
use gpui_component::input::{InputEvent, InputState};
use std::ops::Range;

pub struct NewColumnDialog {
    /// The rule being edited ("Edit"), or `None` for a new one.
    pub editing: Option<u64>,
    /// Row the user selected in (shown first in the Preview).
    pub origin: usize,
    pub candidates: Vec<Candidate>,
    /// The guess picked, or `None` once the regex is the user's own.
    pub chosen: Option<usize>,
    /// Whether the regex field shows.
    pub regex_open: bool,
    pub name_input: Entity<InputState>,
    pub regex_input: Entity<InputState>,
    pub name: PatternField<String>,
    /// "Values will join column X".
    pub name_hint: Option<String>,
    pub pattern: PatternField<PatternError>,
    pub scope: RuleScope,
    /// The Preview of the last usable regex.
    pub preview: Option<Preview>,
    /// The regex typed is not usable: the Preview is of an earlier one.
    pub stale: bool,
    /// The key the chosen guess is anchored on, for the Preview's line not
    /// matched.
    key: Option<String>,
    _subscriptions: Vec<Subscription>,
}

impl NewColumnDialog {
    /// Whether Add column / Save can be pressed.
    pub fn can_save(&self) -> bool {
        self.pattern.is_valid() && self.name.is_valid()
    }

    /// How the regex reads, in words.
    pub fn description(&self) -> String {
        match self.chosen.and_then(|i| self.candidates.get(i)) {
            Some(c) => c.description.clone(),
            None if self.scope == RuleScope::Message => "custom regex on the message".into(),
            None => "custom regex".into(),
        }
    }
}

impl AppView {
    /// "Make column…" on the selection `sel` (bytes of the line) of the row
    /// `row` of the view.
    pub fn open_new_column(&mut self, row: usize, sel: Range<usize>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(&entry_ix) = self.visible_indices.get(row) else { return };
        let Some(entry) = self.all_entries.get(entry_ix) else { return };
        let line = entry.raw_line.to_string();
        let message = entry.spans.get("message").cloned();
        let Some(value) = line.get(sel.clone()).map(str::to_string) else { return };
        let rows: Vec<usize> = sample_rows(self.all_entries.len(), SAMPLE_LINES * 2)
            .into_iter()
            .filter(|&i| !self.all_entries[i].is_continuation())
            .take(SAMPLE_LINES)
            .collect();
        let sample: Vec<(&str, Option<&Range<usize>>)> = rows
            .iter()
            .map(|&i| (self.all_entries[i].raw_line.as_str(), self.all_entries[i].spans.get("message")))
            .collect();
        let candidates = guess(&line, sel, message.as_ref(), &sample);
        let (pattern, scope, key) = match candidates.first() {
            Some(c) => (c.pattern.clone(), c.scope, c.key.clone()),
            None => (format!("({})", regex::escape(&value)), RuleScope::Line, None),
        };
        let name = suggest_name(key.as_deref(), &value);
        let chosen = (!candidates.is_empty()).then_some(0);
        self.show_new_column(None, entry_ix, candidates, chosen, &name, &pattern, scope, key, window, cx);
    }

    /// Where "Make column…" reads from in the Parsed View: text selected
    /// within one line, as (row, bytes of the line).
    pub fn make_column_target(&self) -> Option<(usize, Range<usize>)> {
        let sel = self.text_selection.filter(|s| !s.is_empty())?;
        let (from, to) = sel.ordered();
        if from.row != to.row {
            return None;
        }
        let entry = self.all_entries.get(*self.visible_indices.get(from.row)?)?;
        let range = sel.range_in_row(from.row, entry.raw_line.len())?;
        (!range.is_empty()).then_some((from.row, range))
    }

    /// The same for text selected in one line of the Focus Pane.
    pub fn pane_make_column_target(&self) -> Option<(usize, Range<usize>)> {
        let sel = self.pane_selection.filter(|s| !s.is_empty())?;
        let (from, to) = sel.ordered();
        if from.row != to.row || self.view_mode != crate::workspace::working_set::ViewMode::Parsed {
            return None;
        }
        let crate::focus::FocusRow::Entry(ix) = *self.focus_rows.get(from.row)? else { return None };
        let entry = self.all_entries.get(ix)?;
        let range = sel.range_in_row(from.row, entry.raw_line.len())?;
        let row = self.visible_indices.iter().position(|&i| i == ix)?;
        (!range.is_empty()).then_some((row, range))
    }

    /// "Edit" on a rule of the *Rules* group.
    pub fn open_edit_rule(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(rule) = self.extraction_rules().iter().find(|r| r.id == id).cloned() else { return };
        let origin = self.selected_row_pos.and_then(|p| self.visible_indices.get(p).copied()).unwrap_or(0);
        self.show_new_column(Some(id), origin, Vec::new(), None, &rule.name, &rule.pattern, rule.scope, None, window, cx);
        if let Some(dialog) = self.new_column.as_mut() {
            dialog.regex_open = true;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn show_new_column(
        &mut self,
        editing: Option<u64>,
        origin: usize,
        candidates: Vec<Candidate>,
        chosen: Option<usize>,
        name: &str,
        pattern: &str,
        scope: RuleScope,
        key: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name_input = cx.new(|cx| InputState::new(window, cx).placeholder("Column name").default_value(name.to_string()));
        let regex_input = cx.new(|cx| InputState::new(window, cx).placeholder("e.g. user=(\\S+)").default_value(pattern.to_string()));
        let subscriptions = vec![
            cx.subscribe(&name_input, |this: &mut Self, _, event: &InputEvent, cx| match event {
                InputEvent::Change => this.new_column_name_changed(cx),
                InputEvent::Blur | InputEvent::PressEnter { .. } => {
                    if let Some(d) = this.new_column.as_mut() {
                        d.name.leave();
                        cx.notify();
                    }
                }
                _ => {}
            }),
            cx.subscribe(&regex_input, |this: &mut Self, _, event: &InputEvent, cx| match event {
                InputEvent::Change => this.new_column_regex_changed(cx),
                InputEvent::Blur | InputEvent::PressEnter { .. } => {
                    if let Some(d) = this.new_column.as_mut() {
                        d.pattern.leave();
                        cx.notify();
                    }
                }
                _ => {}
            }),
        ];
        name_input.update(cx, |state, cx| {
            state.focus(window, cx);
            state.select_all(window, cx);
        });
        self.new_column = Some(NewColumnDialog {
            editing,
            origin,
            candidates,
            chosen,
            regex_open: chosen.is_none(),
            name_input,
            regex_input,
            name: PatternField::default(),
            name_hint: None,
            pattern: PatternField::default(),
            scope,
            preview: None,
            stale: false,
            key,
            _subscriptions: subscriptions,
        });
        self.new_column_name_changed(cx);
        self.new_column_regex_changed(cx);
        // The first check of what was filled in shows no error yet.
        cx.notify();
    }

    pub fn close_new_column(&mut self, cx: &mut Context<Self>) {
        if self.new_column.take().is_some() {
            cx.notify();
        }
    }

    /// Names of the Field Columns a new rule could join.
    fn field_names_for_rule(&self, editing: Option<u64>) -> Vec<String> {
        let mut names: Vec<String> = self.filter_index.field_columns().map(|c| c.key.clone()).collect();
        for r in self.extraction_rules() {
            if Some(r.id) != editing {
                names.push(r.name.clone());
            }
        }
        if let Some(own) = editing.and_then(|id| self.extraction_rules().iter().find(|r| r.id == id)) {
            let still_others = self.extraction_rules().iter().any(|r| r.id != own.id && r.name == own.name);
            if !still_others {
                names.retain(|n| *n != own.name);
            }
        }
        names
    }

    fn new_column_name_changed(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.new_column.as_ref() else { return };
        let name = dialog.name_input.read(cx).value().to_string();
        let names = self.field_names_for_rule(dialog.editing);
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let check = check_name(&name, &refs);
        let Some(dialog) = self.new_column.as_mut() else { return };
        dialog.name_hint = matches!(check, NameCheck::Joins(_)).then(|| check.message()).flatten();
        let result = if check.is_ok() { Ok(()) } else { Err(check.message().unwrap_or_default()) };
        let edit = dialog.name.edit(result);
        self.settle_later(edit, false, cx);
        cx.notify();
    }

    fn new_column_regex_changed(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.new_column.as_ref() else { return };
        let text = dialog.regex_input.read(cx).value().to_string();
        let compiled = compile_pattern(&text);
        let Some(dialog) = self.new_column.as_mut() else { return };
        if dialog.chosen.and_then(|i| dialog.candidates.get(i)).is_some_and(|c| c.pattern != text) {
            dialog.chosen = None;
            dialog.key = None;
        }
        let edit = dialog.pattern.edit(compiled.as_ref().map(|_| ()).map_err(Clone::clone));
        match compiled {
            Ok(regex) => {
                let (scope, origin, key) = (dialog.scope, dialog.origin, dialog.key.clone());
                let p = preview(&regex, scope, &self.all_entries, origin, key.as_deref());
                if let Some(dialog) = self.new_column.as_mut() {
                    dialog.preview = Some(p);
                    dialog.stale = false;
                }
            }
            Err(_) => dialog.stale = dialog.preview.is_some(),
        }
        self.settle_later(edit, true, cx);
        cx.notify();
    }

    /// Show the error of the edit `edit` if the user types nothing more for
    /// [`SETTLE`].
    fn settle_later(&mut self, edit: u64, regex: bool, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SETTLE).await;
            this.update(cx, |this, cx| {
                let Some(d) = this.new_column.as_mut() else { return };
                let shown = if regex { d.pattern.settle(edit) } else { d.name.settle(edit) };
                if shown {
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Pick the guess `ix`: its regex goes into the regex field.
    pub fn choose_new_column_candidate(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dialog) = self.new_column.as_mut() else { return };
        let Some(c) = dialog.candidates.get(ix).cloned() else { return };
        dialog.chosen = Some(ix);
        dialog.scope = c.scope;
        dialog.key = c.key.clone();
        let input = dialog.regex_input.clone();
        input.update(cx, |state, cx| state.set_value(c.pattern.clone(), window, cx));
        if let Some(dialog) = self.new_column.as_mut() {
            dialog.chosen = Some(ix);
        }
        self.new_column_regex_changed(cx);
        if let Some(dialog) = self.new_column.as_mut() {
            dialog.chosen = Some(ix);
            dialog.key = c.key;
        }
    }

    pub fn toggle_new_column_regex(&mut self, cx: &mut Context<Self>) {
        if let Some(d) = self.new_column.as_mut() {
            d.regex_open = !d.regex_open;
            cx.notify();
        }
    }

    /// Add column (or Save for "Edit"): only when the name and regex are
    /// usable; otherwise show why.
    pub fn confirm_new_column(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.new_column.as_mut() else { return };
        if !dialog.can_save() {
            dialog.name.leave();
            dialog.pattern.leave();
            cx.notify();
            return;
        }
        let name = dialog.name_input.read(cx).value().trim().to_string();
        let pattern = dialog.regex_input.read(cx).value().to_string();
        let (scope, description, editing) = (dialog.scope, dialog.description(), dialog.editing);
        let result = match editing {
            Some(id) => self.update_extraction_rule(id, &name, &pattern, scope, &description, cx),
            None => self.add_extraction_rule(&name, &pattern, scope, &description, cx),
        };
        if result.is_ok() {
            self.close_new_column(cx);
        }
    }

    /// A line of the Preview was clicked: the view goes to it, the dialog
    /// stays open.
    pub fn jump_to_preview_line(&mut self, entry_ix: usize, cx: &mut Context<Self>) {
        if let Some(pos) = self.visible_indices.iter().position(|&i| i == entry_ix) {
            self.jump_to_row(pos, cx);
        }
    }
}
