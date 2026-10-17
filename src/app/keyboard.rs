//! Logic: keyboard shortcut handling.

use super::AppView;
use crate::ui::floating_panel::PanelKind;
use gpui::{
    Context, KeyDownEvent,
    Window,
};

impl AppView {
    pub(crate) fn handle_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        let cmd = event.keystroke.modifiers.platform || event.keystroke.modifiers.control;
        let shift = event.keystroke.modifiers.shift;
        let alt = event.keystroke.modifiers.alt;

        // Issue 02: Project switcher modal captures keys while open.
        if self.project_switcher_open {
            if !cmd && !alt {
                match key {
                    "Escape" => {
                        self.project_switcher_open = false;
                        self.project_renaming = None;
                        self.project_rename_buffer.clear();
                        cx.notify();
                    }
                    "ArrowDown" => {
                        let count = self.workspace_state.project_count();
                        if count > 0 {
                            self.project_switcher_index = (self.project_switcher_index + 1) % count;
                        }
                        cx.notify();
                    }
                    "ArrowUp" => {
                        let count = self.workspace_state.project_count();
                        if count > 0 {
                            self.project_switcher_index =
                                (self.project_switcher_index + count - 1) % count;
                        }
                        cx.notify();
                    }
                    "Enter" | "enter" => {
                        if self.project_renaming.is_some() {
                            self.commit_project_rename(cx);
                        } else {
                            if let Some(proj) = self
                                .workspace_state
                                .projects
                                .get(self.project_switcher_index)
                            {
                                let id = proj.id.clone();
                                self.switch_project_to(&id, cx);
                            }
                        }
                    }
                    "Backspace" => {
                        if self.project_renaming.is_some() {
                            self.project_rename_buffer.pop();
                            cx.notify();
                        }
                    }
                    other if self.project_renaming.is_some() => {
                        self.project_rename_push_char(other, cx);
                    }
                    _ => {}
                }
            }
            return;
        }

        // Issue 01: Working Set keyboard navigation.
        if cmd && !alt {
            if key.eq_ignore_ascii_case("tab") {
                if shift {
                    self.prev_tab(cx);
                } else {
                    self.next_tab(cx);
                }
                return;
            }
            if !shift {
                if let Ok(digit) = key.parse::<usize>() {
                    if (1..=9).contains(&digit) {
                        self.select_tab_number(digit, cx);
                        return;
                    }
                }
            }
        }

        match (cmd, shift, alt, key) {
            (true, false, false, "f") => {
                self.toggle_search(cx);
                if self.search_state.is_open {
                    self.search_input.update(cx, |state, cx| state.focus(window, cx));
                }
            }
            (true, true, false, "F") => {
                self.toggle_filter_builder(cx);
            }
            (true, true, false, "V") => {
                self.toggle_panel(PanelKind::View, cx);
            }
            (true, true, false, "M") => {
                self.toggle_panel(PanelKind::Marker, cx);
            }
            (true, false, false, "k") => {
                self.is_palette_open = !self.is_palette_open;
                cx.notify();
            }
            (true, false, false, "p") | (true, false, true, "p") => {
                self.toggle_project_switcher(cx);
            }
            (true, false, false, "b") => {
                self.toggle_sidebar(cx);
            }
            (true, false, false, "i") => {
                self.is_inspector_open = !self.is_inspector_open;
                cx.notify();
            }
            (true, false, false, "g") => {
                self.jump_next_match(cx);
            }
            (true, true, false, "G") => {
                self.jump_prev_match(cx);
            }
            (true, false, false, "e") => {
                self.jump_to_next_error(cx);
            }
            (true, true, false, "E") => {
                self.jump_to_prev_error(cx);
            }
            (true, false, false, "w") => {
                self.close_active_tab(cx);
            }
            (true, false, true, "b") => {
                self.toggle_context_sidebar(cx);
            }
            (true, true, false, "B") => {
                self.show_context_tab(crate::workspace::state::ContextTab::Files, cx);
            }
            (true, true, false, "I") => {
                self.show_context_tab(crate::workspace::state::ContextTab::Investigations, cx);
            }
            (true, true, false, "S") => {
                self.show_context_tab(crate::workspace::state::ContextTab::SavedViews, cx);
            }
            (false, false, false, "b") => {
                self.toggle_bookmark_current_line(cx);
            }
            (false, false, false, "m") | (false, false, false, "M") => {
                self.cycle_selected_marker(cx);
            }
            (false, false, false, "f") => {
                // Focus mode on selected tag
                if let Some(entry) = self.selected_entry() {
                    let tag = entry.tag.clone();
                    if self.focused_tag.as_deref() == Some(&tag) {
                        self.focused_tag = None;
                    } else {
                        self.focused_tag = Some(tag);
                    }
                    cx.notify();
                }
            }
            (false, false, false, " ") => {
                self.is_context_open = !self.is_context_open;
                cx.notify();
            }
            (false, false, false, "Enter") | (false, false, false, "enter") => {
                if self.search_state.is_open
                    && !self.search_state.matched_visible_indices.is_empty()
                {
                    self.jump_next_match(cx);
                } else {
                    self.is_context_open = !self.is_context_open;
                    cx.notify();
                }
            }
            (false, true, false, "Enter") | (false, true, false, "enter") => {
                if self.search_state.is_open {
                    self.jump_prev_match(cx);
                }
            }
            (false, false, false, "Escape") => {
                self.search_state.is_open = false;
                self.close_all_panels();
                self.is_context_open = false;
                self.is_palette_open = false;
                self.active_quick_menu = None;
                cx.notify();
            }
            (false, false, false, "ArrowDown") => {
                if let Some(pos) = self.selected_row_pos {
                    if pos + 1 < self.visible_indices.len() {
                        self.jump_to_row(pos + 1, cx);
                    }
                }
            }
            (false, false, false, "ArrowUp") => {
                if let Some(pos) = self.selected_row_pos {
                    if pos > 0 {
                        self.jump_to_row(pos - 1, cx);
                    }
                }
            }
            _ => {}
        }
    }
}
