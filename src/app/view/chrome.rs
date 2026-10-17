//! UI: top window chrome.

use super::AppView;
use crate::theme::ZED;
use gpui::{
    div, px, Context, InteractiveElement, IntoElement,
    ParentElement, StatefulInteractiveElement, Styled,
};
use gpui_component::{Icon, IconName};

impl AppView {
    /// Top window chrome: project switcher trigger, file badge, palette/open buttons.
    pub(super) fn render_window_chrome(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        div()
            .h(px(36.0))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px_3()
            .bg(ZED.surface_background)
            .border_b_1()
            .border_color(ZED.border_variant)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2p5()
                    .child(
                        // Project switcher trigger: color badge + active project name.
                        div()
                            .id("btn_project_switcher")
                            .cursor_pointer()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1p5()
                            .px_2()
                            .py_0p5()
                            .rounded_md()
                            .hover(|s| s.bg(ZED.ghost_hover))
                            .child(self.active_project_swatch())
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(ZED.text)
                                    .child(
                                        self.active_project()
                                            .map(|p| p.name.clone())
                                            .unwrap_or_else(|| "No Project".to_string()),
                                    ),
                            )
                            .child(
                                div()
                                    .text_color(ZED.text_muted)
                                    .child(Icon::new(IconName::ChevronDown).size(px(12.0))),
                            )
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.toggle_project_switcher(cx);
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1p5()
                            .px_2()
                            .py_0p5()
                            .rounded_md()
                            .bg(ZED.element_background)
                            .border_1()
                            .border_color(ZED.border_variant)
                            .text_xs()
                            .text_color(ZED.text_muted)
                            .child(Icon::new(IconName::FileText).size(px(12.0)))
                            .child(self.file_name.clone()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1p5()
                    .child(
                        div()
                            .id("btn_cmd_pal")
                            .cursor_pointer()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1p5()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .hover(|s| s.bg(ZED.ghost_hover))
                            .text_xs()
                            .text_color(ZED.text_muted)
                            .child(
                                div()
                                    .px_1()
                                    .py_0p5()
                                    .rounded_xs()
                                    .bg(ZED.element_background)
                                    .text_color(ZED.text_muted)
                                    .child("⌘K"),
                            )
                            .child("Palette")
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.is_palette_open = !this.is_palette_open;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("btn_open_file")
                            .cursor_pointer()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1p5()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .hover(|s| s.bg(ZED.ghost_hover))
                            .text_xs()
                            .text_color(ZED.text_muted)
                            .child(Icon::new(IconName::FolderOpen).size(px(12.0)))
                            .child("Open...")
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.open_file_dialog(cx);
                            })),
                    ),
            )
            .into_any_element()
    }

    /// Color badge shown in the window chrome next to the active project name.
    pub(super) fn active_project_swatch(&self) -> impl IntoElement {
        let hex = self
            .active_project()
            .map(|p| p.color.hex())
            .unwrap_or("#6B7280");
        let rgb = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0x6B7280);
        div()
            .w(px(10.0))
            .h(px(10.0))
            .rounded_full()
            .bg(gpui::rgb(rgb))
    }
}
