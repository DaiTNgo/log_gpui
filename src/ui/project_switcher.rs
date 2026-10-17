use crate::theme::ZED;
use crate::workspace::project::{Project, ProjectColorAccent};
use gpui::{
    div, px, ElementId, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled,
};

pub struct ProjectSwitcher;

impl ProjectSwitcher {
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        is_open: bool,
        projects: &[Project],
        active_id: Option<&str>,
        selected_index: usize,
        renaming_id: Option<&str>,
        rename_buffer: &str,
        cx: &mut gpui::Context<crate::app::AppView>,
    ) -> gpui::AnyElement {
        if !is_open {
            return div().into_any_element();
        }

        let mut list = div().flex().flex_col().gap_0p5().w_full();

        for (ix, proj) in projects.iter().enumerate() {
            let is_active = active_id == Some(proj.id.as_str());
            let is_selected = selected_index == ix;
            let proj_id = proj.id.clone();
            let proj_name = proj.name.clone();
            let swatch = accent_swatch(proj.color);
            let switch_btn_id = proj_id.clone();
            let rename_btn_id = proj_id.clone();
            let remove_btn_id = proj_id.clone();

            let name_display = if renaming_id == Some(&proj_id) {
                let shown = if rename_buffer.is_empty() {
                    proj_name.clone()
                } else {
                    rename_buffer.to_string()
                };
                div()
                    .text_xs()
                    .font_family(".AppleSystemUIFontMonospaced")
                    .text_color(ZED.text)
                    .child(format!("{}▏", shown))
            } else {
                div()
                    .text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(if is_active { ZED.text_accent } else { ZED.text })
                    .child(proj_name.clone())
            };

            let left = div()
                .id(ElementId::from(format!("proj_switch_{}", proj_id)))
                .cursor_pointer()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .flex_1()
                .px_1()
                .py_1()
                .child(swatch)
                .child(name_display)
                .child(if is_active {
                    div()
                        .px_1()
                        .py_0p5()
                        .rounded_xs()
                        .text_xs()
                        .text_color(ZED.text_muted)
                        .child("ACTIVE")
                        .into_any_element()
                } else {
                    div().into_any_element()
                })
                .on_click(cx.listener(move |this, _, _window, cx| {
                    this.switch_project_to(&switch_btn_id, cx);
                }));

            let row = div()
                .flex()
                .flex_row()
                .items_center()
                .gap_1()
                .px_2()
                .py_1p5()
                .rounded_md()
                .bg(if is_selected {
                    ZED.ghost_selected
                } else if is_active {
                    ZED.element_selected
                } else {
                    ZED.transparent
                })
                .hover(|s| s.bg(if is_selected { ZED.ghost_selected } else { ZED.ghost_hover }))
                .child(left)
                .child(
                    div()
                        .id(ElementId::from(format!("proj_rename_{}", rename_btn_id)))
                        .cursor_pointer()
                        .px_1()
                        .py_0p5()
                        .rounded_md()
                        .text_xs()
                        .text_color(ZED.text_muted)
                        .hover(|s| s.bg(ZED.ghost_hover).text_color(ZED.text))
                        .child("✎")
                        .on_click(cx.listener(move |this, _, _window, cx| {
                            this.begin_project_rename(&rename_btn_id, cx);
                        })),
                )
                .child(
                    div()
                        .id(ElementId::from(format!("proj_remove_{}", remove_btn_id)))
                        .cursor_pointer()
                        .px_1()
                        .py_0p5()
                        .rounded_md()
                        .text_xs()
                        .text_color(ZED.text_muted)
                        .hover(|s| s.bg(ZED.ghost_hover).text_color(ZED.error))
                        .child("×")
                        .on_click(cx.listener(move |this, _, _window, cx| {
                            this.remove_project_at(&remove_btn_id, cx);
                        })),
                );

            list = list.child(row);
        }

        let new_btn_id = "proj_new";
        let footer = div()
            .id(new_btn_id)
            .cursor_pointer()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_3()
            .py_2()
            .mt_1()
            .rounded_md()
            .border_t_1()
            .border_color(ZED.border_variant)
            .text_xs()
            .text_color(ZED.text_accent)
            .hover(|s| s.bg(ZED.ghost_hover))
            .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child("+ New Project"))
            .on_click(cx.listener(|this, _, _window, cx| {
                this.create_new_project(cx);
            }));

        div()
            .absolute()
            .inset_0()
            .bg(gpui::rgba(0x00000088))
            .flex()
            .items_start()
            .justify_center()
            .pt(px(100.0))
            .child(
                div()
                    .w(px(440.0))
                    .bg(ZED.elevated_surface)
                    .border_1()
                    .border_color(ZED.border)
                    .rounded_lg()
                    .shadow_2xl()
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .border_b_1()
                            .border_color(ZED.border_variant)
                            .pb_2()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(ZED.text_muted)
                                    .child("PROJECT SWITCHER"),
                            )
                            .child(
                                div()
                                    .id("proj_esc")
                                    .cursor_pointer()
                                    .text_xs()
                                    .text_color(ZED.text_muted)
                                    .hover(|s| s.text_color(ZED.text))
                                    .child("Esc to Close")
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        this.project_switcher_open = false;
                                        this.project_renaming = None;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(list)
                    .child(footer),
            )
            .into_any_element()
    }
}

/// Color badge swatch for a project. Derives a GPUI color from the accent hex.
fn accent_swatch(accent: ProjectColorAccent) -> impl IntoElement {
    let hex = accent.hex().trim_start_matches('#');
    let rgb = u32::from_str_radix(hex, 16).unwrap_or(0x3B82F6);
    div()
        .w(px(10.0))
        .h(px(10.0))
        .rounded_full()
        .bg(gpui::rgb(rgb))
}