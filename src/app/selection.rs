//! Logic: selected entry, markers, quick filters and error navigation.

use super::AppView;
use crate::marker::MarkerType;
use crate::model::{LogEntry, LogLevel};
use gpui::Context;

impl AppView {
    pub fn remove_filter_condition(&mut self, id: usize, cx: &mut Context<Self>) {
        self.filter_engine.remove_condition(id);
        self.apply_filters_and_refresh(cx);
    }

    pub fn selected_entry(&self) -> Option<&LogEntry> {
        self.selected_row_pos
            .and_then(|pos| self.visible_indices.get(pos))
            .and_then(|&ix| self.all_entries.get(ix))
    }

    pub fn selected_entry_mut(&mut self) -> Option<&mut LogEntry> {
        if let Some(pos) = self.selected_row_pos {
            if let Some(&entry_ix) = self.visible_indices.get(pos) {
                return self.all_entries.get_mut(entry_ix);
            }
        }
        None
    }

    pub fn cycle_selected_marker(&mut self, cx: &mut Context<Self>) {
        if let Some(entry) = self.selected_entry_mut() {
            let next_m = MarkerType::cycle(entry.marker);
            entry.marker = next_m;
            cx.notify();
        }
    }

    pub fn set_selected_marker(&mut self, marker: Option<MarkerType>, cx: &mut Context<Self>) {
        if let Some(entry) = self.selected_entry_mut() {
            entry.marker = marker;
            cx.notify();
        }
    }

    pub fn set_selected_note(&mut self, note: Option<String>, cx: &mut Context<Self>) {
        if let Some(entry) = self.selected_entry_mut() {
            if note.is_some() && entry.marker.is_none() {
                entry.marker = Some(MarkerType::Note);
            }
            entry.note = note;
            cx.notify();
        }
    }

    pub fn add_quick_filter_level(&mut self, level: LogLevel, cx: &mut Context<Self>) {
        self.filter_engine.add_condition(
            crate::filter::ConditionField::Level,
            crate::filter::ConditionOp::Is,
            level.as_str().to_string(),
        );
        self.apply_filters_and_refresh(cx);
    }

    pub fn add_quick_filter_module(&mut self, module: &str, cx: &mut Context<Self>) {
        self.filter_engine.add_condition(
            crate::filter::ConditionField::Module,
            crate::filter::ConditionOp::Is,
            module.to_string(),
        );
        self.apply_filters_and_refresh(cx);
    }

    pub fn toggle_row_expansion(&mut self, entry_ix: usize, cx: &mut Context<Self>) {
        if let Some(entry) = self.all_entries.get_mut(entry_ix) {
            entry.is_expanded = !entry.is_expanded;
            cx.notify();
        }
    }

    pub fn toggle_bookmark_current_line(&mut self, cx: &mut Context<Self>) {
        if let Some(entry) = self.selected_entry_mut() {
            entry.toggle_bookmark();
            cx.notify();
        }
    }

    pub fn jump_to_next_error(&mut self, cx: &mut Context<Self>) {
        let current_pos = self.selected_row_pos.unwrap_or(0);
        let total = self.visible_indices.len();
        for offset in 1..total {
            let next_pos = (current_pos + offset) % total;
            if let Some(&entry_ix) = self.visible_indices.get(next_pos) {
                if let Some(entry) = self.all_entries.get(entry_ix) {
                    if matches!(entry.level, LogLevel::Fatal | LogLevel::Error) {
                        self.jump_to_row(next_pos, cx);
                        break;
                    }
                }
            }
        }
    }

    pub fn jump_to_prev_error(&mut self, cx: &mut Context<Self>) {
        let current_pos = self.selected_row_pos.unwrap_or(0);
        let total = self.visible_indices.len();
        for offset in 1..total {
            let prev_pos = (current_pos + total - offset) % total;
            if let Some(&entry_ix) = self.visible_indices.get(prev_pos) {
                if let Some(entry) = self.all_entries.get(entry_ix) {
                    if matches!(entry.level, LogLevel::Fatal | LogLevel::Error) {
                        self.jump_to_row(prev_pos, cx);
                        break;
                    }
                }
            }
        }
    }
}
