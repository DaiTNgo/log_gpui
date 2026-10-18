//! UI: the New Log Space dialog, the delete confirmation, the notice banner
//! and the Quick Settings panel.

use super::AppView;
use crate::app::ConfirmAction;
use crate::density::Density;
use crate::workspace::new_space::PendingStatus;
use gpui::prelude::FluentBuilder as _;
use gpui::{
    ExternalPaths, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, px,
};
use gpui_component::button::{Button, ButtonGroup, ButtonVariants as _};
use gpui_component::input::Input;
use gpui_component::switch::Switch;
use gpui_component::{
    ActiveTheme as _, Disableable as _, Icon, Selectable as _, Sizable as _, h_flex, v_flex,
};
use gpui_kit::assets::IconName;

impl AppView {
    fn modal_backdrop() -> gpui::Div {
        div()
            .absolute()
            .inset_0()
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
        let create_label = dialog.form.create_label();
        let files = dialog.form.files.clone();
        let name_input = dialog.name_input.clone();

        let mut file_list = v_flex().id("new-space-files").gap_1().max_h(px(180.0)).overflow_y_scroll();
        for (ix, file) in files.iter().enumerate() {
            let name = std::path::Path::new(&file.path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| file.path.clone());
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
                        let color = if file.status == PendingStatus::Binary { danger } else { warning };
                        this.child(div().text_xs().text_color(color).child(warning_text))
                    })
                    .child(
                        Button::new(("new-space-remove", ix))
                            .ghost()
                            .xsmall()
                            .icon(IconName::Close)
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
            .drag_over::<ExternalPaths>(move |style, _, _, _| style.bg(warning.opacity(0.12)))
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                let paths = paths
                    .paths()
                    .iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect();
                this.new_space_add_paths(paths, cx);
            }))
            .child(
                div()
                    .text_sm()
                    .text_color(muted)
                    .child("Drop log files or a folder here"),
            )
            .child(
                Button::new("new-space-browse")
                    .small()
                    .label("Browse…")
                    .on_click(cx.listener(|this, _, _, cx| this.new_space_browse(cx))),
            );

        Self::modal_backdrop()
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
                                this.child(Input::new(&input))
                            })
                            .when_some(name_message, |this, message| {
                                this.child(div().text_xs().text_color(danger).child(message))
                            }),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(div().text_sm().child("Files"))
                            .child(drop_zone)
                            .when(!files.is_empty(), |this| this.child(file_list)),
                    )
                    .child(
                        h_flex()
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
        let (title, detail) = match &action {
            ConfirmAction::DeleteLogSpace { space_id } => {
                let space = self.active_project().and_then(|p| p.log_space(space_id));
                let name = space.map(|s| s.name.clone()).unwrap_or_default();
                let scratch = space
                    .is_some_and(|s| s.kind == crate::workspace::log_space::LogSpaceKind::Scratch);
                (
                    format!("Delete “{name}”?"),
                    if scratch {
                        "Its scratch files will be deleted."
                    } else {
                        "Your log files on disk are not touched."
                    },
                )
            }
            ConfirmAction::DeleteScratchFile { space_id, file_id } => {
                let name = self
                    .active_project()
                    .and_then(|p| p.file(space_id, file_id))
                    .map(|f| f.display_name.clone())
                    .unwrap_or_default();
                (format!("Delete “{name}”?"), "The scratch file will be deleted.")
            }
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
                                    .label("Delete")
                                    .on_click(cx.listener(|this, _, _, cx| this.accept_confirm(cx))),
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

    /// Quick Settings: display switches only (Source Column, density).
    pub(super) fn render_quick_settings(&mut self, cx: &mut gpui::Context<Self>) -> gpui::AnyElement {
        if !self.quick_settings_open {
            return div().into_any_element();
        }
        let theme = cx.theme();
        let (bg, border) = (theme.popover, theme.border);
        let in_space = matches!(
            self.active_target(),
            Some(crate::workspace::working_set::TabTarget::LogSpace { .. })
        );
        let shown = self.source_column_shown();
        let density = self.density;
        let densities = [
            (Density::Comfortable, "Comfortable"),
            (Density::Compact, "Compact"),
            (Density::UltraCompact, "Ultra"),
        ];
        let panel = v_flex()
            .id("quick-settings-panel")
            .absolute()
            .top(px(96.0))
            .right(px(12.0))
            .w(px(260.0))
            .gap_3()
            .p_3()
            .bg(bg)
            .border_1()
            .border_color(border)
            .rounded_lg()
            .shadow_lg()
            .occlude()
            .child(
                Switch::new("show-source")
                    .label("Show source file")
                    .checked(shown)
                    .disabled(!in_space)
                    .on_click(cx.listener(move |this, checked: &bool, _, cx| {
                        this.set_show_source(*checked, cx)
                    })),
            )
            .child(
                v_flex().gap_2().child("Row density").child(
                    ButtonGroup::new("quick-density")
                        .small()
                        .children(densities.map(|(d, label)| {
                            Button::new(SharedString::from(format!("qd-{label}")))
                                .label(label)
                                .selected(density == d)
                        }))
                        .on_click(cx.listener(move |this, ix: &Vec<usize>, _, cx| {
                            if let Some(&i) = ix.first() {
                                this.set_density(densities[i].0, cx);
                            }
                        })),
                ),
            );
        div()
            .absolute()
            .inset_0()
            .id("quick-settings-backdrop")
            .on_click(cx.listener(|this, _, _, cx| {
                this.quick_settings_open = false;
                cx.notify();
            }))
            .child(panel)
            .into_any_element()
    }
}
