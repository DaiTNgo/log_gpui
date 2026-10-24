//! UI: the New Log Space dialog, the delete confirmation, the notice banner
//! and the Quick Settings panel.

use super::AppView;
use crate::app::ConfirmAction;
use crate::workspace::new_space::PendingStatus;
use gpui::prelude::FluentBuilder as _;
use gpui::{
    ExternalPaths, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, px,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::Input;
use gpui_component::{
    ActiveTheme as _, Disableable as _, Icon, Sizable as _, h_flex, v_flex,
};
use gpui_kit::assets::IconName;

/// Why local files cannot be added: the New Log Space has remote members.
const MIX_HINT_LOCAL: &str = "A Log Space holds local or remote files, not both: remove the remote ones to add local files.";
/// Why remote files cannot be added: the New Log Space has local files.
const MIX_HINT_REMOTE: &str = "A Log Space holds local or remote files, not both: remove the local ones to add remote files.";

impl AppView {
    /// The error shown under a form field: an error icon and the message,
    /// in the danger color, so it does not rest on color alone.
    pub(super) fn field_error_row(message: impl Into<gpui::SharedString>, danger: gpui::Hsla) -> gpui::Div {
        h_flex()
            .gap_1()
            .items_center()
            .text_xs()
            .text_color(danger)
            .child(gpui_component::Icon::new(IconName::TriangleAlert).size_3())
            .child(message.into())
    }

    pub(super) fn modal_backdrop() -> gpui::Div {
        div()
            .absolute()
            .inset_0()
            // Swallow mouse and scroll events so nothing under the dialog reacts.
            .occlude()
            .bg(gpui::rgba(0x00000088))
            .flex()
            .items_center()
            .justify_center()
    }

    /// New Log Space: name field, drop zone with Browse, the chosen files,
    /// Create and Cancel. The Create label says what will be made.
    pub(super) fn render_new_space_dialog(&mut self, cx: &mut gpui::Context<Self>) -> gpui::AnyElement {
        let Some(dialog) = self.new_space.as_ref() else {
            return div().into_any_element();
        };
        let Some(project) = self.workspace_state.active_project() else {
            return div().into_any_element();
        };
        let theme = cx.theme();
        let (bg, border, muted, danger, warning) =
            (theme.popover, theme.border, theme.muted_foreground, theme.danger, theme.warning);
        let can_create = dialog.form.can_create(project);
        let name_message = dialog.form.name_message(project);
        // Files first, then Live Globs, as `(server, path or glob, Starting Time)`.
        let mut remote_files: Vec<(String, String, Option<String>)> = self
            .remote
            .new_space_files
            .iter()
            .map(|(server_id, server, path)| {
                let start = self.remote.new_space_starts.get(&(server_id.clone(), path.clone())).cloned();
                (server.clone(), path.clone(), start)
            })
            .collect();
        remote_files.extend(
            self.remote
                .new_space_globs
                .iter()
                .map(|(_, server, dir, pattern)| (server.clone(), crate::remote::browse::join(dir, pattern), None)),
        );
        let remote_commands = self.remote.new_space_commands.clone();
        let remote_count = remote_files.len() + remote_commands.len();
        let create_label = if remote_count == 0 {
            dialog.form.create_label()
        } else {
            crate::workspace::new_space::remote_create_label(
                self.remote.new_space_files.len(),
                self.remote.new_space_globs.len(),
                remote_commands.len(),
            )
        };
        let files = dialog.form.files.clone();
        // Create is off for want of a name only once there is something to
        // create from: say so then.
        let name_hint = dialog.form.name.trim().is_empty() && (!files.is_empty() || remote_count > 0);
        let files_message = dialog.form.files_message();
        let already_listed = dialog.form.already_listed_message();
        let name_input = dialog.name_input.clone();

        let mut file_list = v_flex().id("new-space-files").gap_1().max_h(px(180.0)).overflow_y_scroll();
        for (ix, file) in files.iter().enumerate() {
            let name = file.display_name();
            // Removing an Archive still being read drops what it would list.
            let remove_tip = if file.status == PendingStatus::Reading { "Stop reading" } else { "Remove" };
            file_list = file_list.child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(border)
                    .text_sm()
                    .child(Icon::new(IconName::File).size_4())
                    .child(div().flex_1().truncate().child(name))
                    .when_some(file.warning(), |this, warning_text| {
                        let color = if matches!(
                            file.status,
                            PendingStatus::Binary | PendingStatus::Damaged | PendingStatus::Encrypted
                        ) {
                            danger
                        } else {
                            warning
                        };
                        this.child(div().text_xs().text_color(color).child(warning_text))
                    })
                    .child(
                        Button::new(("new-space-remove", ix))
                            .ghost()
                            .xsmall()
                            .icon(IconName::Close)
                            .tooltip(remove_tip)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.new_space_remove_file(ix, cx)
                            })),
                    ),
            );
        }

        let drop_zone = v_flex()
            .id("new-space-drop")
            .items_center()
            .justify_center()
            .gap_2()
            .h(px(96.0))
            .rounded_md()
            .border_1()
            .border_dashed()
            .border_color(border)
            // The drop itself is taken by the backdrop, once for the dialog.
            .drag_over::<ExternalPaths>(move |style, _, _, _| style.bg(warning.opacity(0.12)))
            .debug_selector(|| "new-space-drop".into())
            .child(
                div()
                    .text_sm()
                    .text_color(muted)
                    .child("Drop log files or a folder here"),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("new-space-browse")
                            .small()
                            .label("Choose files…")
                            .on_click(cx.listener(|this, _, _, cx| this.new_space_browse(false, cx))),
                    )
                    .child(
                        div().debug_selector(|| "new-space-browse-folder".into()).child(
                            Button::new("new-space-browse-folder")
                                .small()
                                .label("Choose folder…")
                                .on_click(cx.listener(|this, _, _, cx| this.new_space_browse(true, cx))),
                        ),
                    ),
            );

        Self::modal_backdrop()
            // The backdrop takes the window's drops while the dialog is open.
            .id("new-space-backdrop")
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                let paths = paths.paths().iter().map(|p| p.to_string_lossy().to_string()).collect();
                this.drop_paths_on_window(paths, cx);
            }))
            .child(
                v_flex()
                    .w(px(520.0))
                    .gap_3()
                    .p_4()
                    .bg(bg)
                    .border_1()
                    .border_color(border)
                    .rounded_lg()
                    .shadow_2xl()
                    .child(div().text_base().font_weight(gpui::FontWeight::SEMIBOLD).child("New Log Space"))
                    .child(
                        v_flex()
                            .gap_1()
                            .child(div().text_sm().child("Name"))
                            .when_some(name_input, |this, input| {
                                let failed = name_message.is_some();
                                this.child(Input::new(&input).when(failed, |i| i.border_color(danger)))
                            })
                            .when_some(name_message, |this, message| {
                                this.child(Self::field_error_row(message, danger))
                            })
                            .when(name_hint, |this| {
                                this.child(
                                    div()
                                        .debug_selector(|| "new-space-name-hint".into())
                                        .text_xs()
                                        .text_color(muted)
                                        .child("Name the Log Space to create it"),
                                )
                            }),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(div().text_sm().child("Files"))
                            .when(remote_count == 0, |this| this.child(drop_zone))
                            .when(remote_count > 0, |this| {
                                this.child(
                                    div()
                                        .debug_selector(|| "new-space-local-hint".into())
                                        .text_xs()
                                        .text_color(muted)
                                        .child(MIX_HINT_LOCAL),
                                )
                            })
                            .when(!files.is_empty() && remote_count == 0, |this| this.child(file_list))
                            .when_some(already_listed, |this, message| {
                                this.child(
                                    div()
                                        .debug_selector(|| "new-space-already-listed".into())
                                        .text_xs()
                                        .text_color(muted)
                                        .child(message),
                                )
                            })
                            .when_some(files_message, |this, message| {
                                // Like the name's error: an icon, not only the color.
                                this.child(
                                    Self::field_error_row(message, danger)
                                        .debug_selector(|| "new-space-files-error".into()),
                                )
                            }),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(div().text_sm().child("Remote files and commands"))
                            .child(
                                // Scrolls like the local list, so Create stays on screen.
                                v_flex()
                                    .id("new-space-remote")
                                    .debug_selector(|| "new-space-remote".into())
                                    .gap_1()
                                    .max_h(px(180.0))
                                    .overflow_y_scroll()
                                    .children(remote_commands.iter().enumerate().map(|(ix, (_, server, command, _))| {
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .px_2()
                                            .py_1()
                                            .rounded_md()
                                            .border_1()
                                            .border_color(border)
                                            .text_sm()
                                            .child(Icon::new(IconName::Globe).size_4())
                                            .child(div().flex_1().truncate().child(format!("{server}$ {command}")))
                                            .child(
                                                Button::new(("new-space-remove-command", ix))
                                                    .ghost()
                                                    .xsmall()
                                                    .icon(IconName::Close)
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        this.new_space_remove_command(ix, cx)
                                                    })),
                                            )
                                    }))
                                    .children(remote_files.iter().enumerate().map(|(ix, (server, path, start))| {
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .px_2()
                                            .py_1()
                                            .rounded_md()
                                            .border_1()
                                            .border_color(border)
                                            .text_sm()
                                            .child(Icon::new(IconName::Globe).size_4())
                                            .child(div().flex_1().truncate().child(format!("{server}:{path}")))
                                            .when_some(start.clone(), |this, start| {
                                                this.child(
                                                    div().text_xs().text_color(muted).child(format!("Starting Time {start}")),
                                                )
                                            })
                                            .child(
                                                Button::new(("new-space-remove-remote", ix))
                                                    .ghost()
                                                    .xsmall()
                                                    .icon(IconName::Close)
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        this.new_space_remove_remote(ix, cx)
                                                    })),
                                            )
                                    })),
                            )
                            .child(
                                Button::new("new-space-add-remote")
                                    .small()
                                    .label("Add remote files…")
                                    .disabled(!files.is_empty())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.open_remote_picker(crate::app::remote_picker::PickerTarget::NewSpace, cx)
                                    })),
                            )
                            .child(
                                Button::new("new-space-add-command")
                                    .small()
                                    .label("Add remote command…")
                                    .disabled(!files.is_empty())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.open_remote_command_picker(
                                            crate::app::remote_picker::PickerTarget::NewSpace,
                                            cx,
                                        )
                                    })),
                            )
                            .when(!files.is_empty(), |this| {
                                this.child(
                                    div()
                                        .debug_selector(|| "new-space-remote-hint".into())
                                        .text_xs()
                                        .text_color(muted)
                                        .child(MIX_HINT_REMOTE),
                                )
                            }),
                    )
                    .when(files.is_empty() && remote_count == 0, |this| {
                        // What "Create empty" makes: a Scratch Log Space.
                        this.child(
                            div()
                                .debug_selector(|| "new-space-scratch-hint".into())
                                .text_xs()
                                .text_color(muted)
                                .child("With no files, the Log Space starts with an empty file to paste or type log text into."),
                        )
                    })
                    .child(
                        h_flex()
                            .debug_selector(|| "new-space-footer".into())
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("new-space-cancel")
                                    .label("Cancel")
                                    .on_click(cx.listener(|this, _, _, cx| this.new_space_cancel(cx))),
                            )
                            .child(
                                Button::new("new-space-create")
                                    .primary()
                                    .label(create_label)
                                    .disabled(!can_create)
                                    .on_click(cx.listener(|this, _, _, cx| this.new_space_create(cx))),
                            ),
                    ),
            )
            .into_any_element()
    }

    /// Confirmation before deleting a Log Space or a scratch file.
    pub(super) fn render_confirm(&mut self, cx: &mut gpui::Context<Self>) -> gpui::AnyElement {
        let Some(action) = self.confirm.clone() else {
            return div().into_any_element();
        };
        let (title, detail): (String, String) = match &action {
            ConfirmAction::ClearFiles { markers, .. } => (
                format!("Clear will remove {markers} Marker{}", if *markers == 1 { "" } else { "s" }),
                "Markers do not come back when the lines are loaded again.".into(),
            ),
            ConfirmAction::DeleteLogSpace { space_id } => {
                let space = self.active_project().and_then(|p| p.log_space(space_id));
                let name = space.map(|s| s.name.clone()).unwrap_or_default();
                let scratch = space
                    .is_some_and(|s| s.kind == crate::workspace::log_space::LogSpaceKind::Scratch);
                (
                    format!("Delete “{name}”?"),
                    if scratch {
                        "Its scratch files will be deleted.".into()
                    } else {
                        "Your log files on disk are not touched.".into()
                    },
                )
            }
            ConfirmAction::DeleteServer { server_id } => {
                let name = self.remote.servers.get(server_id).map(|s| s.name.clone()).unwrap_or_default();
                (
                    format!("Delete Server \u{201c}{name}\u{201d}?"),
                    "Its settings and remembered password are removed. Files on it are not touched.".into(),
                )
            }
            ConfirmAction::RemoveProject { project_id } => {
                let name = self
                    .workspace_state
                    .projects
                    .iter()
                    .find(|p| p.id == *project_id)
                    .map(|p| p.name.clone())
                    .unwrap_or_default();
                (
                    format!("Remove \u{201c}{name}\u{201d} from the Sidebar?"),
                    "Its Log Spaces are removed with it. Log files on disk are not touched.".into(),
                )
            }
            ConfirmAction::DeleteScratchFile { space_id, file_id } => {
                let name = self
                    .active_project()
                    .and_then(|p| p.file(space_id, file_id))
                    .map(|f| f.display_name.clone())
                    .unwrap_or_default();
                (format!("Delete “{name}”?"), "The scratch file will be deleted.".into())
            }
            ConfirmAction::DeleteLogFormat { format_id } => {
                let name = self.formats.store.get(format_id).map(|f| f.name.clone()).unwrap_or_default();
                let several = self.workspace_state.projects.len() > 1;
                let places: Vec<String> = self
                    .log_format_uses(format_id)
                    .into_iter()
                    .map(|u| if several { format!("{} ({})", u.place, u.project) } else { u.place })
                    .collect();
                (
                    format!("Delete Log Format \u{201c}{name}\u{201d}?"),
                    format!("Used by {}. They go back to Auto-detect.", places.join(", ")),
                )
            }
        };
        let confirm_label = match action {
            ConfirmAction::ClearFiles { .. } => "Clear",
            ConfirmAction::RemoveProject { .. } => "Remove",
            _ => "Delete",
        };
        let theme = cx.theme();
        let (bg, border, muted) = (theme.popover, theme.border, theme.muted_foreground);
        Self::modal_backdrop()
            .child(
                v_flex()
                    .w(px(400.0))
                    .gap_3()
                    .p_4()
                    .bg(bg)
                    .border_1()
                    .border_color(border)
                    .rounded_lg()
                    .shadow_2xl()
                    .child(div().text_base().font_weight(gpui::FontWeight::SEMIBOLD).child(title))
                    .child(div().text_sm().text_color(muted).child(detail))
                    .child(
                        h_flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("confirm-cancel")
                                    .label("Cancel")
                                    .on_click(cx.listener(|this, _, _, cx| this.cancel_confirm(cx))),
                            )
                            .child(
                                Button::new("confirm-delete")
                                    .danger()
                                    .label(confirm_label)
                                    .on_click(cx.listener(|this, _, _, cx| this.accept_confirm(cx))),
                            ),
                    ),
            )
            .into_any_element()
    }

    /// Load more with a number of lines the user types.
    pub(super) fn render_custom_load(&mut self, cx: &mut gpui::Context<Self>) -> gpui::AnyElement {
        let Some(custom) = self.custom_load.as_ref() else {
            return div().into_any_element();
        };
        let input = custom.input.clone();
        let remaining = self.hold_info(&custom.file_id).map_or(0, |i| i.remaining);
        let theme = cx.theme();
        let (bg, border, muted) = (theme.popover, theme.border, theme.muted_foreground);
        Self::modal_backdrop()
            .child(
                v_flex()
                    .w(px(400.0))
                    .gap_3()
                    .p_4()
                    .bg(bg)
                    .border_1()
                    .border_color(border)
                    .rounded_lg()
                    .shadow_2xl()
                    .child(div().text_base().font_weight(gpui::FontWeight::SEMIBOLD).child("Load more lines"))
                    .child(div().text_sm().text_color(muted).child(format!(
                        "How many older lines to read from the Server (up to {} more fit).",
                        crate::app::live_more::group_digits(remaining)
                    )))
                    .child(Input::new(&input))
                    .child(
                        h_flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("custom-load-cancel")
                                    .label("Cancel")
                                    .on_click(cx.listener(|this, _, _, cx| this.cancel_custom_load(cx))),
                            )
                            .child(
                                Button::new("custom-load-accept")
                                    .primary()
                                    .label("Load")
                                    .on_click(cx.listener(|this, _, _, cx| this.accept_custom_load(cx))),
                            ),
                    ),
            )
            .into_any_element()
    }

    /// A light message above the status bar.
    pub(super) fn render_notice(&mut self, cx: &mut gpui::Context<Self>) -> gpui::AnyElement {
        let Some(message) = self.notice.clone() else {
            return div().into_any_element();
        };
        let theme = cx.theme();
        let (bg, border) = (theme.popover, theme.border);
        h_flex()
            .w_full()
            .px_3()
            .py_1()
            .gap_2()
            .items_center()
            .bg(bg)
            .border_t_1()
            .border_color(border)
            .text_sm()
            .child(div().flex_1().child(message))
            .child(
                Button::new("notice-dismiss")
                    .ghost()
                    .xsmall()
                    .icon(IconName::Close)
                    .on_click(cx.listener(|this, _, _, cx| this.dismiss_notice(cx))),
            )
            .into_any_element()
    }
}
