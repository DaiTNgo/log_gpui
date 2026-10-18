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
        let current_line_num = self.current_line().map(|l| l.line).unwrap_or(0);
        // How many lines the open Focus Tab lists, out of every line.
        let focus_label = format!("Focus {} of {}", self.focus_rows.len(), total_count);
        let bar = StatusBar::new()
            .left(format!("Ln {}", current_line_num))
            .left(format!("{},{:03} events", total_count / 1000, total_count % 1000))
            .left(focus_label)
            .left("UTF-8");

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
