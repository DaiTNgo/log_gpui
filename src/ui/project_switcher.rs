
use crate::workspace::project::{Project, ProjectColorAccent};
use gpui::prelude::FluentBuilder as _;
use gpui_component::{ActiveTheme as _, Icon};
use gpui_kit::assets::IconName;
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
        _selected_index: usize,
        renaming_id: Option<&str>,
        rename_buffer: &str,
        cx: &mut gpui::Context<crate::app::AppView>,
    ) -> gpui::AnyElement {
        if !is_open {
            return div().into_any_element();
        }

        let mono_font = cx.theme().mono_font_family.clone();
        // The last Project cannot be removed.
        let can_remove = projects.len() > 1;
        let mut list = div().flex().flex_col().gap_0p5().w_full();

        for proj in projects.iter() {
            let is_active = active_id == Some(proj.id.as_str());
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
                    .flex()
                    .items_center()
                    .text_xs()
                    .font_family(mono_font.clone())
                    .child(shown)
                    // The caret is drawn, not a glyph the font may lack.
                    .child(div().w(px(1.0)).h(px(12.0)).ml(px(1.0)).bg(cx.theme().foreground))
            } else {
                div()
                    .text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
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
                .child(left)
                .child(
                    div()
                        .id(ElementId::from(format!("proj_rename_{}", rename_btn_id)))
                        .cursor_pointer()
                        .px_1()
                        .py_0p5()
                        .rounded_md()
                        .text_xs()
                        .child(Icon::new(IconName::Pencil).size(px(12.0)))
                        .on_click(cx.listener(move |this, _, _window, cx| {
                            this.begin_project_rename(&rename_btn_id, cx);
                        })),
                )
                .child(
                    div()
                        .id(ElementId::from(format!("proj_remove_{}", remove_btn_id)))
                        .px_1()
                        .py_0p5()
                        .rounded_md()
                        .text_xs()
                        .child("×")
                        .map(|this| {
                            if can_remove {
                                this.cursor_pointer()
                                    .text_color(cx.theme().danger)
                                    .tooltip(|window, cx| gpui_component::tooltip::Tooltip::new("Remove from Sidebar").build(window, cx))
                                    .on_click(cx.listener(move |this, _, _window, cx| {
                                        this.request_remove_project(&remove_btn_id, cx);
                                    }))
                            } else {
                                this.opacity(0.4).tooltip(|window, cx| {
                                    gpui_component::tooltip::Tooltip::new("The last Project cannot be removed")
                                        .build(window, cx)
                                })
                            }
                        }),
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
            .text_xs()
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
                    .border_1()
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
                            .pb_2()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child("PROJECT SWITCHER"),
                            )
                            .child(
                                div()
                                    .id("proj_esc")
                                    .cursor_pointer()
                                    .text_xs()
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