//! UI: status bar built from the gpui-kit `StatusBar`.

use super::AppView;
use crate::app::format_count;
use gpui::{Context, IntoElement};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::popover::Popover;
use gpui_component::status_bar::StatusBar;
use gpui_component::{Disableable as _, Selectable as _, Sizable as _};

impl AppView {
    /// The live state of the open Log Space as one label: the worst status of
    /// its files with how many have it, and the list of every file.
    fn live_status_label(&self) -> Option<(String, String)> {
        if let Some(server) = self.server_waiting_for_agent() {
            let text = format!("Waiting for agent approval: {server}");
            return Some((text.clone(), text));
        }
        let (worst, count, total) = self.worst_live_status()?;
        let label =
            crate::live::connection_label(&worst, count, total, self.slowest_server_latency());
        let list = self
            .live_file_statuses()
            .into_iter()
            .map(|(name, status)| format!("{name}: {}", status.label()))
            .collect::<Vec<_>>()
            .join("\n");
        Some((label, list))
    }

    /// The Remote Copy of the open Log Space being read: its file and the
    /// progress text for the status bar.
    fn copy_read_progress(&self) -> Option<(String, String)> {
        let space = self.active_space()?;
        space.files.iter().find_map(|f| {
            let read = self.live.copy_reads.get(&f.file_id)?;
            let mut text = read.progress_text(&f.display_name);
            if read.numbers_unavailable {
                text.push_str(" — line numbers unavailable");
            }
            Some((f.file_id.clone(), text))
        })
    }

    /// "Reading app.log.2.gz 45% (17 / 38 MB)" for a long read of a file of
    /// the open Log Space.
    fn read_progress_text(&self) -> Option<String> {
        let space = self.active_space()?;
        let progress = space.files.iter().find_map(|f| self.live.progress.get(&f.file_id))?;
        let size = crate::live::buffer::size_text;
        Some(match progress.total.filter(|t| *t > 0) {
            Some(total) => format!(
                "Reading {} {}% ({} / {})",
                progress.what,
                (progress.done.min(total) * 100 / total),
                size(progress.done),
                size(total)
            ),
            None => format!("Reading {} ({})", progress.what, size(progress.done)),
        })
    }

    /// Load more, Clear and Back to Live Window of the only file of the open
    /// Log Space, which has no file row to hold them. Each shows only for the
    /// kind of file it applies to (a Remote Command only has Clear).
    fn single_file_live_buttons(&self, mut bar: StatusBar, cx: &mut Context<Self>) -> StatusBar {
        let Some(file) = self.active_space().and_then(|s| s.single_file()) else { return bar };
        let file_id = file.file_id.clone();
        let Some(info) = self.hold_info(&file_id) else { return bar };
        let step = self.live.load_more_step;
        bar = bar.left(Button::new("status-file-name").ghost().xsmall().label(file.display_name.clone()).tooltip(info.tooltip()));
        if !info.command {
            let id = file_id.clone();
            let at_limit = info.load_more_off == Some(crate::app::live_more::AT_FULL_READ_LIMIT);
            let off = (info.load_more_off.is_some() && !at_limit) || info.loading;
            bar = bar.right(
                Button::new("status-load-more")
                    .ghost()
                    .xsmall()
                    .disabled(off)
                    .label(format!("Load more ({})", info.amount_label(step)))
                    .on_click(cx.listener(move |this, _, _window, cx| this.load_more(&id, None, cx))),
            );
            if info.loading {
                let id = file_id.clone();
                bar = bar.right(
                    Button::new("status-cancel-load")
                        .ghost()
                        .xsmall()
                        .label("Cancel loading")
                        .on_click(cx.listener(move |this, _, _window, _cx| this.cancel_load_more(&id))),
                );
            }
            let id = file_id.clone();
            bar = bar.right(
                Button::new("status-back-to-live")
                    .ghost()
                    .xsmall()
                    .disabled(!info.extended)
                    .label("Back to Live Window")
                    .on_click(cx.listener(move |this, _, _window, cx| this.back_to_live_window(&id, cx))),
            );
        }
        if !info.local {
            let id = file_id;
            bar = bar.right(
                Button::new("status-clear")
                    .ghost()
                    .xsmall()
                    .label("Clear")
                    .on_click(cx.listener(move |this, _, _window, cx| this.request_clear_file(&id, cx))),
            );
        }
        bar
    }

    /// The plain labels on the left of the status bar, in order.
    pub fn status_bar_labels(&self) -> Vec<String> {
        let total_count = self.all_entries.len();
        let current_line_num = self.current_line().map(|l| l.line).unwrap_or(0);
        vec![
            format!("Ln {current_line_num}"),
            format!("{} lines", format_count(total_count)),
            // How many lines the open Focus Tab lists, out of every line.
            format!("Focus {} of {}", self.focus_rows.len(), total_count),
            "UTF-8".to_string(),
        ]
    }

    /// Status bar: line number, event counts, filter ratio, focus tag, and the
    /// live state (Follow, Pause and the connection) of the open Log Space.
    pub(super) fn render_status_bar(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let mut bar = StatusBar::new();
        for label in self.status_bar_labels() {
            bar = bar.left(label);
        }

        if !self.active_space_is_live() {
            return bar.into_any_element();
        }

        if let Some((label, list)) = self.live_status_label() {
            let trigger = Button::new("status-live").ghost().xsmall().label(label);
            bar = match self.connection_snapshot() {
                // Connection Details opens from here (L7).
                Some(snapshot) => {
                    let view = cx.entity();
                    bar.right(Popover::new("connection-details").trigger(trigger).content(
                        move |_, _, cx| {
                            super::connection_view::connection_details(&snapshot, &view, cx)
                        },
                    ))
                }
                None => bar.right(trigger.tooltip(list)),
            };
        }
        let pending = self.glob_pending_notes();
        if !pending.is_empty() {
            bar = bar.right(
                Button::new("status-glob-pending")
                    .ghost()
                    .xsmall()
                    .label(pending[0].clone())
                    .tooltip(pending.join("\n")),
            );
        }
        if let Some((file_id, text)) = self.copy_read_progress() {
            bar = bar
                .right(Button::new("status-read-progress").ghost().xsmall().label(text))
                .right(
                    Button::new("status-read-cancel")
                        .ghost()
                        .xsmall()
                        .label("Cancel")
                        .on_click(cx.listener(move |this, _, _window, cx| {
                            this.cancel_copy_read(&file_id);
                            cx.notify();
                        })),
                );
        } else if let Some(text) = self.read_progress_text() {
            bar = bar.right(Button::new("status-read-progress").ghost().xsmall().label(text));
        }
        if let Some((label, action)) = self.connect_button() {
            bar = bar.right(
                Button::new("status-connect")
                    .ghost()
                    .xsmall()
                    .label(label)
                    .on_click(cx.listener(move |this, _, _window, cx| {
                        this.press_connect(action.clone(), cx)
                    })),
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
        bar = self.single_file_live_buttons(bar, cx);
        bar.right(
            Button::new("status-pause")
                .ghost()
                .xsmall()
                .selected(self.active_space_paused())
                .label(if self.active_space_paused() {
                    "Paused"
                } else {
                    "Pause"
                })
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
