//! Logic: palette command dispatch.

use super::AppView;
use crate::density::Density;
use crate::ui::command_palette::PaletteAction;
use gpui::{
    Context,
};

impl AppView {
    pub fn execute_palette_action(&mut self, action: PaletteAction, cx: &mut Context<Self>) {
        self.is_palette_open = false;
        match action {
            PaletteAction::ToggleSearch => self.toggle_search(cx),
            PaletteAction::ShowHighlights => self.toggle_or_reveal_context_section(crate::workspace::state::ContextSection::Highlights, cx),
            PaletteAction::ToggleFocusPane => self.toggle_focus_pane(cx),
            PaletteAction::ToggleFilter => self.toggle_filter_builder(cx),
            PaletteAction::ShowContext => {
                self.is_context_open = true;
                cx.notify();
            }
            PaletteAction::GoNextError => self.jump_to_next_error(cx),
            PaletteAction::GoPrevError => self.jump_to_prev_error(cx),
            PaletteAction::SetComfortable => {
                self.density = Density::Comfortable;
                self.column_layout.update_density(self.density);
                cx.notify();
            }
            PaletteAction::SetCompact => {
                self.density = Density::Compact;
                self.column_layout.update_density(self.density);
                cx.notify();
            }
            PaletteAction::SetUltraCompact => {
                self.density = Density::UltraCompact;
                self.column_layout.update_density(self.density);
                cx.notify();
            }
            PaletteAction::ToggleTail => {
                self.tail_enabled = !self.tail_enabled;
                cx.notify();
            }
            PaletteAction::ToggleInspector => {
                self.toggle_or_reveal_context_section(crate::workspace::state::ContextSection::Inspector, cx);
            }
            PaletteAction::LoadDemo => {
                self.load_demo(cx);
            }
            PaletteAction::NewLogSpace => {
                self.open_new_space_dialog(cx);
            }
            PaletteAction::ToggleRawParsed => {
                self.toggle_view_mode(cx);
            }
            PaletteAction::SaveScratch => {
                self.save_now(cx);
            }
            PaletteAction::UnpinAllColumns => {
                self.unpin_all_columns(cx);
            }
            PaletteAction::ToggleProjectSwitcher => {
                self.toggle_project_switcher(cx);
            }
        }
    }
}
