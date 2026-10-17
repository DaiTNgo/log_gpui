//! UI: status bar built from the gpui-kit `StatusBar`.

use super::AppView;
use gpui::{Context, IntoElement};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::status_bar::StatusBar;
use gpui_component::Sizable as _;

impl AppView {
    /// Status bar: line number, event counts, filter ratio, focus tag and tail toggle.
    pub(super) fn render_status_bar(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let total_count = self.all_entries.len();
        let visible_count = self.visible_indices.len();
        let filter_pct = if total_count > 0 {
            100.0 - (visible_count as f64 / total_count as f64) * 100.0
        } else {
            0.0
        };
        let current_line_num = self.selected_entry().map(|e| e.index).unwrap_or(0);

        let filter_label = if filter_pct > 0.0 {
            format!("Filtered {:.1}%", filter_pct)
        } else {
            "All Visible".to_string()
        };

        let mut bar = StatusBar::new()
            .left(format!("Ln {}", current_line_num))
            .left(format!("{},{:03} events", total_count / 1000, total_count % 1000))
            .left(filter_label)
            .left("UTF-8");
        if let Some(tag) = &self.focused_tag {
            bar = bar.left(format!("Focus: [{}]", tag));
        }

        bar.right(
            Button::new("status-tail")
                .ghost()
                .xsmall()
                .label(if self.tail_enabled { "Tail: ON" } else { "Tail: OFF" })
                .on_click(cx.listener(|this, _, _window, cx| this.toggle_tail(cx))),
        )
        .into_any_element()
    }
}
