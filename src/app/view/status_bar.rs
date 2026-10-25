//! UI: status bar built from the gpui-kit `StatusBar`.

use super::AppView;
use crate::app::format_count;
use crate::keymap::Command;
use crate::live::StatusTone;
use gpui::prelude::FluentBuilder as _;
use gpui::{Context, IntoElement, Styled as _};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::popover::Popover;
use gpui_component::status_bar::StatusBar;
use gpui_component::{ActiveTheme as _, Disableable as _, IconName, Selectable as _, Sizable as _};

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
        if info.offers_load_more() {
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
        }
        if !info.command {
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

    /// The plain labels on the left of the status bar, in order: the
    /// current line (named by its file in a Log Space of several files) when
    /// there is one, the line count, and how many lines the open Focus Tab
    /// lists while the Focus Pane shows them.
    pub fn status_bar_labels(&self) -> Vec<String> {
        let mut labels = Vec::new();
        if let Some(line) = self.current_line() {
            let number = format!("Ln {}", format_count(line.line));
            let file = self
                .active_space()
                .filter(|s| s.files.len() > 1)
                .and_then(|s| s.files.iter().find(|f| *f.file_id == *line.file_id))
                .map(|f| f.display_name.clone());
            labels.push(match file {
                Some(name) => format!("{name} · {number}"),
                None => number,
            });
        }
        labels.push(format!("{} lines", format_count(self.view.len())));
        if self.focus_pane_shows_lines() {
            let n = self.focus_rows.len();
            labels.push(format!("{} {} in Focus", format_count(n), if n == 1 { "line" } else { "lines" }));
        }
        labels
    }

    /// The words on the Follow and Pause controls: "Following" or "Follow",
    /// "Resume" while paused or "Pause".
    pub fn live_control_labels(&self) -> (String, String) {
        let follow = if self.live.follow { "Following" } else { "Follow" };
        let pause = if self.active_space_paused() { "Resume" } else { "Pause" };
        (follow.into(), pause.into())
    }

    /// `text`, with the first Shortcut of `command` when it has one.
    fn with_shortcut(&self, text: &str, command: Command) -> String {
        match self.keymap.shortcuts(command).first() {
            Some(shortcut) => format!("{text} ({})", crate::app::shortcuts::shortcut_label(shortcut)),
            None => text.to_string(),
        }
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
            let worst = self.worst_live_status().map(|(status, ..)| status.tone());
            let theme = cx.theme();
            let (icon, color) = match worst {
                Some(StatusTone::Good) => (IconName::CircleCheck, theme.success),
                Some(StatusTone::Warning) => (IconName::RefreshCw, theme.warning),
                Some(StatusTone::Bad) => (IconName::CircleX, theme.danger),
                Some(StatusTone::Busy) | None => (IconName::LoaderCircle, theme.muted_foreground),
            };
            let trigger = Button::new("status-live").ghost().xsmall().icon(icon).label(label).text_color(color);
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
        let (follow, pause) = self.live_control_labels();
        let paused = self.active_space_paused();
        let pause_tip = self.with_shortcut(
            if paused { "Take in the lines written while paused" } else { "Stop taking in new lines" },
            Command::TogglePause,
        );
        let follow_tip = self.with_shortcut(
            if self.live.follow { "Keeping the newest line in sight" } else { "Scroll to each new line as it arrives" },
            Command::ToggleFollow,
        );
        let warning = cx.theme().warning;
        bar.right(
            Button::new("status-pause")
                .ghost()
                .xsmall()
                .selected(paused)
                .icon(if paused { IconName::Play } else { IconName::Pause })
                .label(pause)
                .when(paused, |b| b.text_color(warning))
                .tooltip(pause_tip)
                .on_click(cx.listener(|this, _, _window, cx| this.toggle_pause(cx))),
        )
        .right(
            Button::new("status-follow")
                .ghost()
                .xsmall()
                .selected(self.live.follow)
                .icon(IconName::ArrowDown)
                .label(follow)
                .tooltip(follow_tip)
                .on_click(cx.listener(|this, _, _window, cx| this.toggle_follow(cx))),
        )
        .into_any_element()
    }
}
