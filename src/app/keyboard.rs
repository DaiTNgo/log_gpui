//! Logic: keyboard shortcut handling.

use super::AppView;
use gpui::{
    Context, KeyDownEvent,
    Window,
};

impl AppView {
    /// Close the topmost open dialog as its Cancel would, innermost first
    /// (the Confirm above everything, the Server form before the Servers
    /// list). Returns whether one was open.
    pub fn close_top_dialog(&mut self, cx: &mut Context<Self>) -> bool {
        if self.confirm.is_some() {
            self.cancel_confirm(cx);
        } else if self.remote.sign_in_dialog.is_some() {
            self.answer_sign_in(None, cx);
        } else if !self.remote.host_prompts.is_empty() {
            self.answer_host_key(false, cx);
        } else if self.remote.secret_prompt.is_some() {
            self.cancel_secret_prompt(cx);
        } else if let Some(dialog) = self.server_dialog.as_ref() {
            if dialog.import.is_some() {
                self.close_ssh_import(cx);
            } else if dialog.form.is_some() {
                self.cancel_server_form(cx);
            } else {
                self.close_servers_dialog(cx);
            }
        } else if self.new_space.is_some() {
            self.new_space_cancel(cx);
        } else {
            return false;
        }
        true
    }

    pub(crate) fn handle_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        // gpui names the key "escape"; the arms below spell it "Escape".
        let key = match event.keystroke.key.as_str() {
            "escape" => "Escape",
            other => other,
        };
        let cmd = event.keystroke.modifiers.platform || event.keystroke.modifiers.control;
        let shift = event.keystroke.modifiers.shift;
        let alt = event.keystroke.modifiers.alt;

        // Escape closes the topmost dialog as its Cancel, and goes no further.
        if key == "Escape" && self.close_top_dialog(cx) {
            return;
        }
        // The New Log Space dialog and the delete confirmation capture keys while open.
        if self.new_space.is_some() {
            return;
        }
        if self.confirm.is_some() {
            if matches!(key, "Enter" | "enter") {
                self.accept_confirm(cx);
            }
            return;
        }

        // Project switcher modal captures keys while open.
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

        // Raw View: only Search and Markers apply, and plain keys belong to the editors.
        if self.view_mode == crate::workspace::working_set::ViewMode::Raw && !cmd {
            if key == "Escape" {
                self.focus.search_mut().is_open = false;
                self.is_palette_open = false;
                cx.notify();
            }
            return;
        }

        let shortcut = crate::keymap::Shortcut::from_keystroke(&event.keystroke);
        match (cmd, shift, alt, key) {
            (true, false, false, "c") if self.pane_selection.is_some() => {
                self.copy_pane_selection(cx);
                return;
            }
            (true, false, false, "c") if self.view_mode == crate::workspace::working_set::ViewMode::Raw => {
                self.copy_selected_raw_line(cx);
                return;
            }
            (true, false, false, "c") if self.text_selection.is_some() => {
                self.copy_text_selection(cx);
                return;
            }
            _ => {}
        }

        // The Keymap. A Shortcut without ⌘ or ⌥ pressed in a text field or menu
        // belongs to it (the search field's own PressEnter subscription steps
        // through the Matches), and steps through Matches only while the
        // Search is open.
        if let Some(command) = self.keymap.command_for(&shortcut) {
            let in_child = window.focused(cx).is_some_and(|handle| handle != self.focus_handle);
            let steps = matches!(command, crate::keymap::Command::NextMatch | crate::keymap::Command::PrevMatch);
            if !shortcut.is_plain() || (!in_child && (!steps || self.focus.search().is_open)) {
                self.execute_command(command, window, cx);
            }
            return;
        }

        match (cmd, shift, alt, key) {
            (false, false, false, "Escape") => {
                self.clear_text_selection(cx);
                self.focus.search_mut().is_open = false;
                self.close_all_panels();
                self.is_palette_open = false;
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
