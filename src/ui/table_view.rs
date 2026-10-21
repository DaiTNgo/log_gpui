use crate::model::{LogEntry, LogLevel};
use gpui::{
    div, px, rgb, App, Context, Div, ElementId, InteractiveElement, IntoElement,
    ParentElement, Stateful, Styled, Window,
};
use gpui_component::{
    table::{Column, TableDelegate, TableState},
    ActiveTheme,
};
use std::sync::Arc;

pub struct LogTableDelegate {
    pub entries: Arc<Vec<LogEntry>>,
    pub visible_indices: Arc<Vec<usize>>,
    pub columns: Vec<Column>,
}

impl LogTableDelegate {
    pub fn new(entries: Arc<Vec<LogEntry>>, visible_indices: Arc<Vec<usize>>) -> Self {
        let columns = vec![
            Column::new("index", "#").width(px(65.0)),
            Column::new("time", "Time").width(px(140.0)),
            Column::new("delta", "Δt (ms)").width(px(85.0)),
            Column::new("tag", "Tag / Service").width(px(120.0)),
            Column::new("level", "Level").width(px(75.0)),
            Column::new("message", "Message").width(px(700.0)),
        ];

        Self {
            entries,
            visible_indices,
            columns,
        }
    }

    pub fn get_entry(&self, row_ix: usize) -> Option<&LogEntry> {
        let actual_ix = self.visible_indices.get(row_ix)?;
        self.entries.get(*actual_ix)
    }
}

impl TableDelegate for LogTableDelegate {
    fn columns_count(&self, _cx: &App) -> usize {
        self.columns.len()
    }

    fn rows_count(&self, _cx: &App) -> usize {
        self.visible_indices.len()
    }

    fn column(&self, col_ix: usize, _cx: &App) -> Column {
        self.columns[col_ix].clone()
    }

    fn render_tr(
        &mut self,
        row_ix: usize,
        _window: &mut Window,
        _cx: &mut Context<TableState<Self>>,
    ) -> Stateful<Div> {
        let mut tr = div().id(ElementId::from(row_ix));
        if let Some(entry) = self.get_entry(row_ix) {
            if let Some(ref color) = entry.marker_color {
                let bg = match color.as_str() {
                    "red" => rgb(0x451a1a),
                    "yellow" => rgb(0x422006),
                    "blue" => rgb(0x172554),
                    "green" => rgb(0x052e16),
                    "purple" => rgb(0x3b0764),
                    _ => rgb(0x27272a),
                };
                tr = tr.bg(bg);
            }
        }
        tr
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let Some(entry) = self.get_entry(row_ix) else {
            return div().into_any_element();
        };

        match col_ix {
            0 => div()
                .px_2()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(entry.index.to_string())
                .into_any_element(),
            1 => div()
                .px_2()
                .font_family(cx.theme().mono_font_family.clone())
                .text_xs()
                .child(entry.time_str().to_string())
                .into_any_element(),
            2 => {
                let delta_text = entry
                    .delta_time_ms
                    .map(|d| format!("+{:.2}", d))
                    .unwrap_or_else(|| "-".to_string());
                div()
                    .px_2()
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_xs()
                    .text_color(rgb(0x38bdf8))
                    .child(delta_text)
                    .into_any_element()
            }
            3 => div()
                .px_2()
                .text_xs()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(rgb(0xa78bfa))
                .child(entry.tag().to_string())
                .into_any_element(),
            4 => {
                let (color, bg) = match entry.level {
                    LogLevel::Fatal => (rgb(0xffffff), rgb(0x991b1b)),
                    LogLevel::Error => (rgb(0xfca5a5), rgb(0x450a0a)),
                    LogLevel::Warn => (rgb(0xfde047), rgb(0x422006)),
                    LogLevel::Info => (rgb(0x93c5fd), rgb(0x172554)),
                    LogLevel::Debug => (rgb(0x86efac), rgb(0x052e16)),
                    LogLevel::Verbose => (rgb(0xd1d5db), rgb(0x1f2937)),
                    LogLevel::Unknown => (rgb(0x9ca3af), rgb(0x1f2937)),
                };
                div()
                    .px_1()
                    .py_0p5()
                    .rounded_sm()
                    .text_xs()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(color)
                    .bg(bg)
                    .child(entry.level.short_code())
                    .into_any_element()
            }
            5 => {
                let text_color: gpui::Rgba = match entry.level {
                    LogLevel::Fatal | LogLevel::Error => rgb(0xf87171),
                    LogLevel::Warn => rgb(0xfbbf24),
                    _ => cx.theme().foreground.into(),
                };
                div()
                    .px_2()
                    .text_xs()
                    .text_color(text_color)
                    .truncate()
                    .child(entry.payload().to_string())
                    .into_any_element()
            }
            _ => div().into_any_element(),
        }
    }
}
