//! Logic: running a Command, from the Command Palette or a Shortcut.

use super::AppView;
use crate::density::Density;
use crate::keymap::Command;
use gpui::{Context, Window};

impl AppView {
    pub fn execute_command(&mut self, command: Command, window: &mut Window, cx: &mut Context<Self>) {
        let palette_was_open = self.is_palette_open;
        self.is_palette_open = false;
        match command {
            Command::NextMatch => self.jump_next_match(cx),
            Command::PrevMatch => self.jump_prev_match(cx),
            Command::ToggleSidebar => self.toggle_sidebar(cx),
            Command::ToggleContextSidebar => self.toggle_context_sidebar(cx),
            Command::CloseTab => self.close_active_tab(cx),
            Command::CloseOtherTabs => {
                if let Some(idx) = self.active_project().and_then(|p| p.working_set.active_tab_index()) {
                    self.close_scope_at(crate::workspace::CloseScope::Others, idx, cx);
                }
            }
            Command::CommandPalette => {
                self.is_palette_open = !palette_was_open;
                cx.notify();
            }
            Command::OpenSettings => crate::ui::settings::open_settings_dialog(cx.entity(), window, cx),
            Command::OpenKeymap => {
                self.settings_section = crate::ui::settings::SettingsSection::Keymap;
                crate::ui::settings::open_settings_dialog(cx.entity(), window, cx);
            }
            Command::ToggleSearch => self.activate_search(window, cx),
            Command::ShowHighlights => self.toggle_or_reveal_context_section(crate::workspace::state::ContextSection::Highlights, cx),
            Command::ToggleFocusPane => self.toggle_focus_pane(cx),
            Command::MoveFocusToWindow => self.move_focus_to_window(window, cx),
            Command::ToggleFindFilter => self.toggle_find_filter(cx),
            Command::ToggleSearchHighlight => self.toggle_search_highlight(cx),
            Command::ToggleFilter => self.toggle_filter_builder(cx),
            Command::GoNextError => self.jump_to_next_error(cx),
            Command::GoPrevError => self.jump_to_prev_error(cx),
            Command::SetComfortable => {
                self.density = Density::Comfortable;
                self.column_layout.update_density(self.density);
                cx.notify();
            }
            Command::SetCompact => {
                self.density = Density::Compact;
                self.column_layout.update_density(self.density);
                cx.notify();
            }
            Command::SetUltraCompact => {
                self.density = Density::UltraCompact;
                self.column_layout.update_density(self.density);
                cx.notify();
            }
            Command::ManageServers => self.open_servers_dialog(cx),
            Command::ToggleFollow => self.toggle_follow(cx),
            Command::TogglePause => self.toggle_pause(cx),
            Command::ClearFile => self.request_clear_current_file(cx),
            Command::LoadMore => self.load_more_current_file(cx),
            Command::BackToLiveWindow => self.back_to_live_window_current_file(cx),
            Command::OpenInFull => self.open_in_full_current(cx),
            Command::ToggleInspector => {
                self.toggle_or_reveal_context_section(crate::workspace::state::ContextSection::Inspector, cx);
            }
            Command::LoadDemo => {
                self.load_demo(cx);
            }
            Command::NewLogSpace => {
                self.open_new_space_dialog(cx);
            }
            Command::ToggleRawParsed => {
                self.toggle_view_mode(cx);
            }
            Command::SaveScratch => {
                self.save_now(cx);
            }
            Command::UnpinAllColumns => {
                self.unpin_all_columns(cx);
            }
            Command::ToggleProjectSwitcher => {
                self.toggle_project_switcher(cx);
            }
        }
    }
}
