//! UI: status bar built from the gpui-kit `StatusBar`.

use super::AppView;
use gpui::{Context, IntoElement};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::popover::Popover;
use gpui_component::status_bar::StatusBar;
use gpui_component::{Selectable as _, Sizable as _};

impl AppView {
    /// The live state of the open Log Space as one label: the worst status of
    /// its files with how many have it, and the list of every file.
    fn live_status_label(&self) -> Option<(String, String)> {
        let (worst, count, total) = self.worst_live_status()?;
        let label = crate::live::connection_label(&worst, count, total, self.slowest_server_latency());
        let list = self
            .live_file_statuses()
            .into_iter()
            .map(|(name, status)| format!("{name}: {}", status.label()))
            .collect::<Vec<_>>()
            .join("\n");
        Some((label, list))
    }

    /// Status bar: line number, event counts, filter ratio, focus tag, and the
    /// live state (Follow, Pause and the connection) of the open Log Space.
    pub(super) fn render_status_bar(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let total_count = self.all_entries.len();
        let current_line_num = self.current_line().map(|l| l.line).unwrap_or(0);
        // How many lines the open Focus Tab lists, out of every line.
        let focus_label = format!("Focus {} of {}", self.focus_rows.len(), total_count);
        let mut bar = StatusBar::new()
            .left(format!("Ln {}", current_line_num))
            .left(format!("{},{:03} events", total_count / 1000, total_count % 1000))
            .left(focus_label)
            .left("UTF-8");

        if !self.active_space_is_live() {
            return bar.into_any_element();
        }

        if let Some((label, list)) = self.live_status_label() {
            let trigger = Button::new("status-live").ghost().xsmall().label(label);
            bar = match self.connection_snapshot() {
                // Connection Details opens from here (L7).
                Some(snapshot) => {
                    let view = cx.entity();
                    bar.right(
                        Popover::new("connection-details")
                            .trigger(trigger)
                            .content(move |_, _, cx| super::connection_view::connection_details(&snapshot, &view, cx)),
                    )
                }
                None => bar.right(trigger.tooltip(list)),
            };
        }
        if let Some((label, action)) = self.connect_button() {
            bar = bar.right(
                Button::new("status-connect")
                    .ghost()
                    .xsmall()
                    .label(label)
                    .on_click(cx.listener(move |this, _, _window, cx| this.press_connect(action.clone(), cx))),
            );
        }
        if !self.live.follow && self.live.unseen > 0 {
            bar = bar.right(
                Button::new("status-unseen")
                    .ghost()
                    .xsmall()
                    .label(format!("+{} new", self.live.unseen))
                    .on_click(cx.listener(|this, _, _window, cx| this.set_follow(true, cx))),
            );
        }
        bar.right(
            Button::new("status-pause")
                .ghost()
                .xsmall()
                .selected(self.active_space_paused())
                .label(if self.active_space_paused() { "Paused" } else { "Pause" })
                .on_click(cx.listener(|this, _, _window, cx| this.toggle_pause(cx))),
        )
        .right(
            Button::new("status-follow")
                .ghost()
                .xsmall()
                .selected(self.live.follow)
                .label("Follow")
                .on_click(cx.listener(|this, _, _window, cx| this.toggle_follow(cx))),
        )
        .into_any_element()
    }
}
