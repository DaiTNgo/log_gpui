//! Logic: palette command dispatch and file opening.

use super::AppView;
use crate::column::FieldMaxChars;
use crate::parser::parse_file;
use crate::sample::generate_sample_logs;
use crate::theme::Density;
use crate::ui::command_palette::PaletteAction;
use crate::ui::floating_panel::PanelKind;
use gpui::{
    Context,
};

impl AppView {
    pub fn execute_palette_action(&mut self, action: PaletteAction, cx: &mut Context<Self>) {
        self.is_palette_open = false;
        match action {
            PaletteAction::ToggleSearch => self.toggle_search(cx),
            PaletteAction::ToggleFilter => self.toggle_filter_builder(cx),
            PaletteAction::ToggleViewPanel => self.toggle_panel(PanelKind::View, cx),
            PaletteAction::ToggleMarkerPanel => self.toggle_panel(PanelKind::Marker, cx),
            PaletteAction::ToggleBookmark => self.toggle_bookmark_current_line(cx),
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
                self.is_inspector_open = !self.is_inspector_open;
                cx.notify();
            }
            PaletteAction::LoadDemo => {
                let demo = generate_sample_logs();
                let max_chars = FieldMaxChars::scan_entries(&demo);
                self.open_log_file_in_tab(
                    "app.log".to_string(),
                    "app.log".to_string(),
                    demo,
                    max_chars,
                    cx,
                );
                self.file_format = "Standard Server Log".to_string();
            }
            PaletteAction::OpenFile => {
                self.open_file_dialog(cx);
            }
            PaletteAction::UnpinAllColumns => {
                self.unpin_all_columns(cx);
            }
            PaletteAction::ToggleProjectSwitcher => {
                self.toggle_project_switcher(cx);
            }
        }
    }

    pub fn open_file_dialog(&mut self, cx: &mut Context<Self>) {
        if let Some(file_path) = rfd::FileDialog::new()
            .add_filter("Log Files (*.log, *.txt, *.json)", &["log", "txt", "json", "jsonl"])
            .pick_file()
        {
            let name = file_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "Unknown File".to_string());

            match parse_file(&file_path) {
                Ok(result) => {
                    let file_id = file_path.to_string_lossy().to_string();
                    self.column_layout.max_chars = result.max_chars;
                    self.open_log_file_in_tab(
                        file_id,
                        name,
                        result.entries,
                        result.max_chars,
                        cx,
                    );
                    self.file_format = "Text Log".to_string();
                }
                Err(e) => {
                    eprintln!("Failed to parse file: {:?}", e);
                }
            }
        }
    }
}
