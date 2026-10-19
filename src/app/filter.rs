//! Logic: Filter actions (values, custom values, Hide Mode, Time Range) and the
//! state of the Filter section's inputs.

use super::AppView;
use crate::filter::parse_time_bound;
use gpui::{AppContext as _, Context, Entity, SharedString, Subscription, Window};
use gpui_component::input::{InputEvent, InputState};

/// UI-only state of the Filter section in the context sidebar.
pub struct FilterUi {
    /// The "Add filter" column list is open.
    pub picker_open: bool,
    /// Column whose values are being edited (at most one at a time).
    pub expanded: Option<String>,
    /// "Add value" input of the expanded column.
    pub value_input: Entity<InputState>,
    pub from_input: Entity<InputState>,
    pub to_input: Entity<InputState>,
    /// The Time Range inputs must be rewritten from the Filter (set when the
    /// Filter is replaced, e.g. after switching file); done at render time.
    pub inputs_dirty: bool,
    _subscriptions: Vec<Subscription>,
}

impl FilterUi {
    pub fn new(window: &mut Window, cx: &mut Context<AppView>) -> Self {
        let value_input = cx.new(|cx| InputState::new(window, cx).placeholder("Add value…"));
        let from_input = cx.new(|cx| InputState::new(window, cx).placeholder("From"));
        let to_input = cx.new(|cx| InputState::new(window, cx).placeholder("To"));

        let mut subscriptions = Vec::new();
        subscriptions.push(cx.subscribe_in(
            &value_input,
            window,
            |this: &mut AppView, input, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    let text = input.read(cx).value().to_string();
                    if this.filter_add_expanded_custom(&text, cx) {
                        input.update(cx, |state, cx| {
                            state.set_value(SharedString::from(""), window, cx)
                        });
                    }
                }
            },
        ));
        for input in [&from_input, &to_input] {
            subscriptions.push(cx.subscribe_in(
                input,
                window,
                |this: &mut AppView, _, event: &InputEvent, _window, cx| {
                    if matches!(event, InputEvent::Change | InputEvent::PressEnter { .. }) {
                        this.on_time_inputs_changed(cx);
                    }
                },
            ));
        }

        Self {
            picker_open: false,
            expanded: None,
            value_input,
            from_input,
            to_input,
            inputs_dirty: true,
            _subscriptions: subscriptions,
        }
    }
}

impl AppView {
    /// Re-evaluate the Filter on the whole file, then refresh Search and the
    /// Focus Pane. The view keeps every line: Filter only narrows the pane.
    pub fn apply_filters_and_refresh(&mut self, cx: &mut Context<Self>) {
        let pass = self
            .filter_state
            .evaluate(&self.filter_index, &mut self.all_entries);
        self.filter_pass = std::sync::Arc::new(pass);
        if self.visible_indices.len() != self.all_entries.len() {
            // Always every row in order: grow or cut the list where it is, not
            // a new one of the size of the file for every batch of live lines.
            let rows = self.all_entries.len();
            let visible = std::sync::Arc::make_mut(&mut self.visible_indices);
            let have = visible.len();
            if have < rows {
                visible.extend(have..rows);
            } else {
                visible.truncate(rows);
            }
        }
        self.longest_row = longest(self.all_entries.iter().map(|e| e.payload.len()));
        // Rows may be new: a selection of them no longer means the same.
        self.text_selection = None;
        self.selecting = false;
        self.recompute_highlight_paints();
        self.focus
            .search_mut()
            .update_matches(&self.all_entries, &self.filter_pass);
        self.refresh_focus();

        if let Some(pos) = self.selected_row_pos {
            if pos >= self.visible_indices.len() {
                self.selected_row_pos = if self.visible_indices.is_empty() {
                    None
                } else {
                    Some(0)
                };
            }
        }
        self.schedule_session_save(cx);
        cx.notify();
    }

    /// Detect the columns and values of the loaded file. Call whenever
    /// `all_entries` is replaced.
    pub fn rebuild_filter_index(&mut self) {
        self.filter_index = crate::filter_index::FilterIndex::build(&self.all_entries);
        self.filter_ui.picker_open = false;
        self.filter_ui.expanded = None;
        self.filter_ui.inputs_dirty = true;
    }

    pub fn toggle_filter_picker(&mut self, cx: &mut Context<Self>) {
        self.filter_ui.picker_open = !self.filter_ui.picker_open;
        cx.notify();
    }

    /// Open a column for editing (or close it when it is already open).
    pub fn toggle_filter_column(&mut self, key: &str, cx: &mut Context<Self>) {
        let open = self.filter_ui.expanded.as_deref() == Some(key);
        self.filter_ui.expanded = if open { None } else { Some(key.to_string()) };
        self.filter_ui.picker_open = false;
        cx.notify();
    }

    /// Choose a column from the "Add filter" list. Columns above the distinct
    /// value limit are not available.
    pub fn pick_filter_column(&mut self, key: &str, cx: &mut Context<Self>) {
        let available = self.filter_index.column(key).is_some_and(|c| c.is_filterable());
        if available {
            self.filter_ui.expanded = Some(key.to_string());
            self.filter_ui.picker_open = false;
            cx.notify();
        }
    }

    pub fn filter_toggle_value(&mut self, key: &str, value: &str, cx: &mut Context<Self>) {
        self.filter_state.toggle_value(key, value);
        self.apply_filters_and_refresh(cx);
    }

    /// Add the typed text as a Custom Value of the expanded column.
    pub fn filter_add_expanded_custom(&mut self, text: &str, cx: &mut Context<Self>) -> bool {
        let Some(key) = self.filter_ui.expanded.clone() else {
            return false;
        };
        let added = self.filter_state.add_custom(&key, text);
        if added {
            self.apply_filters_and_refresh(cx);
        }
        added
    }

    pub fn filter_remove_custom(&mut self, key: &str, text: &str, cx: &mut Context<Self>) {
        self.filter_state.remove_custom(key, text);
        self.apply_filters_and_refresh(cx);
    }

    pub fn filter_set_hide(&mut self, key: &str, hide: bool, cx: &mut Context<Self>) {
        self.filter_state.set_hide(key, hide);
        self.apply_filters_and_refresh(cx);
    }

    pub fn filter_clear_column(&mut self, key: &str, cx: &mut Context<Self>) {
        self.filter_state.clear_column(key);
        if self.filter_ui.expanded.as_deref() == Some(key) {
            self.filter_ui.expanded = None;
        }
        self.apply_filters_and_refresh(cx);
    }

    pub fn filter_clear_all(&mut self, cx: &mut Context<Self>) {
        self.filter_state.clear();
        self.filter_ui.expanded = None;
        self.filter_ui.inputs_dirty = true;
        self.apply_filters_and_refresh(cx);
    }

    /// The From/To inputs changed: store the typed bounds and re-filter.
    pub fn on_time_inputs_changed(&mut self, cx: &mut Context<Self>) {
        let from = self.filter_ui.from_input.read(cx).value().to_string();
        let to = self.filter_ui.to_input.read(cx).value().to_string();
        let (from, to) = (from.trim().to_string(), to.trim().to_string());
        if from == self.filter_state.time_range.from && to == self.filter_state.time_range.to {
            return;
        }
        self.filter_state.set_time_range(&from, &to);
        self.apply_filters_and_refresh(cx);
    }

    /// Whether a typed time bound can be read; empty text is fine.
    pub fn time_bound_is_valid(&self, text: &str) -> bool {
        parse_time_bound(text, self.filter_index.anchor_day()).is_ok()
    }

    /// Rewrite the Time Range inputs from the Filter and refresh their hints.
    /// Called from `render`, where a window is available.
    pub fn sync_filter_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.filter_ui.inputs_dirty {
            return;
        }
        self.filter_ui.inputs_dirty = false;
        let (from, to) = (
            self.filter_state.time_range.from.clone(),
            self.filter_state.time_range.to.clone(),
        );
        let (from_hint, to_hint) = match self.filter_index.time_hint() {
            Some((a, b)) => (a, b),
            None => ("From".to_string(), "To".to_string()),
        };
        self.filter_ui.from_input.update(cx, |state, cx| {
            state.set_placeholder(from_hint, window, cx);
            state.set_value(SharedString::from(from), window, cx);
        });
        self.filter_ui.to_input.update(cx, |state, cx| {
            state.set_placeholder(to_hint, window, cx);
            state.set_value(SharedString::from(to), window, cx);
        });
    }
}

/// Position of the largest of `lens` (the first one on a tie), 0 when empty.
pub(crate) fn longest(lens: impl Iterator<Item = usize>) -> usize {
    let mut best = (0, 0);
    for (pos, len) in lens.enumerate() {
        if len > best.1 {
            best = (pos, len);
        }
    }
    best.0
}
