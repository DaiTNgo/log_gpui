use crate::theme::ZED;
use crate::ui::icons::{
    render_icon, CARET_DOWN_SVG, CARET_RIGHT_SVG, FILE_TEXT_SVG, FOLDER_OPEN_SVG, FOLDER_SVG,
    PLUS_SVG, X_SVG,
};
use crate::workspace::project::ProjectColorAccent;
use crate::workspace::state::WorkspaceState;
use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, ElementId, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled,
};
use std::collections::HashSet;

pub struct WorkspaceTree;

impl WorkspaceTree {
    pub fn render(
        workspace: &WorkspaceState,
        active_project_id: Option<&str>,
        active_file_id: Option<&str>,
        expanded_projects: &HashSet<String>,
        cx: &mut gpui::Context<crate::app::AppView>,
    ) -> gpui::AnyElement {
        let mut tree = div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .overflow_hidden();

        // Tree content list
        let mut list = div()
            .flex()
            .flex_col()
            .w_full()
            .py_1()
            .gap_0p5();

        for project in &workspace.projects {
            let is_active = active_project_id == Some(project.id.as_str());
            let is_expanded = expanded_projects.contains(&project.id);
            let project_id = project.id.clone();
            let project_id_for_caret = project.id.clone();
            let project_id_for_row = project.id.clone();
            let project_id_for_add = project.id.clone();
            let file_count = project.files.len();

            // Project Row
            let caret_svg = if is_expanded {
                CARET_DOWN_SVG
            } else {
                CARET_RIGHT_SVG
            };

            let folder_svg = if is_expanded {
                FOLDER_OPEN_SVG
            } else {
                FOLDER_SVG
            };

            let project_row = div()
                .id(ElementId::from(format!("tree_proj_{}", project_id)))
                .cursor_pointer()
                .group("project_row")
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .h(px(26.0))
                .px_2()
                .mx_1()
                .rounded_sm()
                .bg(if is_active {
                    ZED.ghost_selected
                } else {
                    ZED.transparent
                })
                .hover(|s| s.bg(ZED.ghost_hover))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_1p5()
                        .overflow_hidden()
                        // Caret toggle button
                        .child(
                            div()
                                .id(ElementId::from(format!("caret_proj_{}", project_id)))
                                .cursor_pointer()
                                .w(px(14.0))
                                .h(px(14.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_sm()
                                .hover(|s| s.bg(ZED.element_hover))
                                .child(render_icon(caret_svg, 11.0, ZED.icon_muted))
                                .on_click(cx.listener(move |this, _, _window, cx| {
                                    cx.stop_propagation();
                                    this.toggle_project_expand(&project_id_for_caret, cx);
                                })),
                        )
                        // Folder Icon
                        .child(
                            div()
                                .w(px(14.0))
                                .h(px(14.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(render_icon(
                                    folder_svg,
                                    13.0,
                                    if is_active {
                                        ZED.icon_accent
                                    } else {
                                        ZED.icon
                                    },
                                )),
                        )
                        // Color Accent Dot
                        .child(accent_swatch(project.color))
                        // Project Name
                        .child(
                            div()
                                .text_xs()
                                .font_weight(if is_active {
                                    gpui::FontWeight::SEMIBOLD
                                } else {
                                    gpui::FontWeight::MEDIUM
                                })
                                .text_color(if is_active {
                                    ZED.text
                                } else {
                                    ZED.text
                                })
                                .child(project.name.clone()),
                        ),
                )
                // Right badges / actions
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_1()
                        // Quick Add File button
                        .child(
                            div()
                                .id(ElementId::from(format!("proj_add_file_{}", project_id)))
                                .cursor_pointer()
                                .w(px(16.0))
                                .h(px(16.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_sm()
                                .hover(|s| s.bg(ZED.element_hover))
                                .child(render_icon(PLUS_SVG, 10.0, ZED.icon_muted))
                                .on_click(cx.listener(move |this, _, _window, cx| {
                                    cx.stop_propagation();
                                    this.switch_project_to(&project_id_for_add, cx);
                                    this.open_file_dialog(cx);
                                })),
                        )
                        // File count pill
                        .child(
                            div()
                                .px_1()
                                .py_0p5()
                                .rounded_sm()
                                .bg(ZED.element_background)
                                .text_color(ZED.text_muted)
                                .text_xs()
                                .child(format!("{}", file_count)),
                        ),
                )
                .on_click(cx.listener(move |this, _, _window, cx| {
                    this.switch_project_to(&project_id_for_row, cx);
                    if !this.expanded_projects.contains(&project_id_for_row) {
                        this.expanded_projects.insert(project_id_for_row.clone());
                    }
                }));

            list = list.child(project_row);

            // Child Files
            if is_expanded {
                let mut files_container = div()
                    .flex()
                    .flex_col()
                    .w_full()
                    .py_0p5()
                    .gap_0p5();

                for file in &project.files {
                    let is_file_active = active_file_id == Some(file.file_id.as_str());
                    let is_file_open = project.working_set.is_file_open(&file.file_id);
                    let file_id = file.file_id.clone();
                    let file_id_for_close = file.file_id.clone();

                    let file_row = div()
                        .id(ElementId::from(format!("tree_file_{}", file_id)))
                        .cursor_pointer()
                        .group("file_row")
                        .relative()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .h(px(24.0))
                        .pl_7()
                        .pr_2()
                        .mx_1()
                        .rounded_sm()
                        .bg(if is_file_active {
                            ZED.element_selected
                        } else {
                            ZED.transparent
                        })
                        .hover(|s| {
                            if is_file_active {
                                s.bg(ZED.element_selected)
                            } else {
                                s.bg(ZED.ghost_hover)
                            }
                        })
                        // Left active indicator border bar
                        .when(is_file_active, |this| {
                            this.border_l_2()
                                .border_color(ZED.border_focused)
                        })
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_1p5()
                                .overflow_hidden()
                                // Log file icon
                                .child(
                                    div()
                                        .w(px(14.0))
                                        .h(px(14.0))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(render_icon(
                                            FILE_TEXT_SVG,
                                            12.0,
                                            if is_file_active {
                                                ZED.icon_accent
                                            } else if is_file_open {
                                                ZED.icon
                                            } else {
                                                ZED.icon_muted
                                            },
                                        )),
                                )
                                // File Display Name
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(if is_file_active {
                                            gpui::FontWeight::MEDIUM
                                        } else {
                                            gpui::FontWeight::NORMAL
                                        })
                                        .text_color(if is_file_active {
                                            ZED.text_accent
                                        } else if is_file_open {
                                            ZED.text
                                        } else {
                                            ZED.text_muted
                                        })
                                        .child(file.display_name.clone()),
                                ),
                        )
                        // Right status indicator & quick close tab button
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_1()
                                .when(is_file_open, |this| {
                                    this.child(
                                        div()
                                            .id(ElementId::from(format!("close_file_{}", file_id)))
                                            .cursor_pointer()
                                            .w(px(16.0))
                                            .h(px(16.0))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .rounded_sm()
                                            .hover(|s| s.bg(ZED.element_hover))
                                            .child(render_icon(X_SVG, 10.0, ZED.icon_muted))
                                            .on_click(cx.listener(move |this, _, _window, cx| {
                                                cx.stop_propagation();
                                                this.close_tab_for_file(&file_id_for_close, cx);
                                            })),
                                    )
                                })
                                .when(is_file_active, |this| {
                                    this.child(
                                        div()
                                            .w(px(5.0))
                                            .h(px(5.0))
                                            .rounded_full()
                                            .bg(ZED.icon_accent),
                                    )
                                })
                                .when(is_file_open && !is_file_active, |this| {
                                    this.child(
                                        div()
                                            .w(px(5.0))
                                            .h(px(5.0))
                                            .rounded_full()
                                            .border_1()
                                            .border_color(ZED.icon_muted),
                                    )
                                }),
                        )
                        .on_click(cx.listener(move |this, _, _window, cx| {
                            this.open_file_from_sidebar(&file_id, cx);
                        }));

                    files_container = files_container.child(file_row);
                }

                // If project is expanded but has no files
                if project.files.is_empty() {
                    let proj_id_for_add = project.id.clone();
                    files_container = files_container.child(
                        div()
                            .id(ElementId::from(format!("empty_add_file_{}", project.id)))
                            .cursor_pointer()
                            .pl_7()
                            .pr_2()
                            .py_1()
                            .text_xs()
                            .text_color(ZED.text_muted)
                            .hover(|s| s.text_color(ZED.text_accent))
                            .child("+ Add log file...")
                            .on_click(cx.listener(move |this, _, _window, cx| {
                                this.switch_project_to(&proj_id_for_add, cx);
                                this.open_file_dialog(cx);
                            })),
                    );
                }

                list = list.child(files_container);
            }
        }

        // Empty state when workspace has no projects
        if workspace.projects.is_empty() {
            let empty_state = div()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .text_center()
                .p_4()
                .m_2()
                .gap_2()
                .rounded_md()
                .border_1()
                .border_dashed()
                .border_color(ZED.border_variant)
                .bg(ZED.surface_background)
                .child(
                    div()
                        .w(px(32.0))
                        .h(px(32.0))
                        .rounded_full()
                        .bg(ZED.element_background)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(render_icon(FOLDER_SVG, 18.0, ZED.icon_muted)),
                )
                .child(
                    div()
                        .text_xs()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(ZED.text)
                        .child("No Projects in Workspace"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(ZED.text_muted)
                        .child("Create a project to organize and analyze your log files."),
                )
                .child(
                    div()
                        .id(ElementId::from("btn_empty_create_proj"))
                        .cursor_pointer()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_1()
                        .px_2p5()
                        .py_1()
                        .rounded_sm()
                        .bg(ZED.element_background)
                        .border_1()
                        .border_color(ZED.border)
                        .hover(|s| s.bg(ZED.element_hover))
                        .child(render_icon(PLUS_SVG, 12.0, ZED.icon_accent))
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(ZED.text)
                                .child("New Project"),
                        )
                        .on_click(cx.listener(|this, _, _window, cx| {
                            this.toggle_project_switcher(cx);
                        })),
                );

            list = list.child(empty_state);
        }

        tree = tree.child(list);
        tree.into_any_element()
    }
}

fn accent_swatch(accent: ProjectColorAccent) -> impl IntoElement {
    let hex = accent.hex().trim_start_matches('#');
    let rgb = u32::from_str_radix(hex, 16).unwrap_or(0x3B82F6);
    div()
        .w(px(6.0))
        .h(px(6.0))
        .rounded_full()
        .bg(gpui::rgb(rgb))
}
