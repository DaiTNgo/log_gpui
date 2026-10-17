use crate::theme::{Density, ZED};
use crate::ui::icons::{
    render_icon, ARROW_COUNTER_CLOCKWISE_SVG, ARROWS_IN_LINE_VERTICAL_SVG, BOOKMARK_SIMPLE_SVG,
    FLAG_SVG, FOLDER_OPEN_SVG, FOLDER_SVG, GEAR_SIX_SVG, PLUS_SVG, SLIDERS_HORIZONTAL_SVG,
};
use crate::ui::workspace_tree::WorkspaceTree;
use crate::workspace::state::{ActivityRailTab, SidebarState, SidebarStateMachine, WorkspaceState};
use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, AppContext, ElementId, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Render, StatefulInteractiveElement, Styled, Window,
};
use std::collections::HashSet;

pub const RAIL_WIDTH: f32 = 40.0;

#[derive(Clone)]
pub struct ResizeDrawer;

impl Render for ResizeDrawer {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
    }
}
pub struct SidebarView;

impl SidebarView {
    pub fn render(
        sidebar: &SidebarStateMachine,
        workspace: &WorkspaceState,
        expanded_projects: &HashSet<String>,
        all_entries: &[crate::model::LogEntry],
        visible_indices: &[usize],
        density: Density,
        tail_enabled: bool,
        cx: &mut gpui::Context<crate::app::AppView>,
    ) -> gpui::AnyElement {
        match sidebar.state {
            SidebarState::Hidden => div().into_any_element(),
            SidebarState::Compact => Self::render_rail(sidebar, cx),
            SidebarState::Expanded => {
                let mut el = div().flex().flex_row().h_full();
                el = el.child(Self::render_rail(sidebar, cx));
                el = el.child(Self::render_drawer(
                    sidebar,
                    workspace,
                    expanded_projects,
                    all_entries,
                    visible_indices,
                    density,
                    tail_enabled,
                    cx,
                ));
                el.into_any_element()
            }
        }
    }

    fn render_rail(
        sidebar: &SidebarStateMachine,
        cx: &mut gpui::Context<crate::app::AppView>,
    ) -> gpui::AnyElement {
        let top_tabs = [
            (ActivityRailTab::Files, FOLDER_SVG, "Explorer"),
            (ActivityRailTab::Investigations, FLAG_SVG, "Investigations"),
            (ActivityRailTab::SavedViews, SLIDERS_HORIZONTAL_SVG, "Saved Views"),
            (ActivityRailTab::Markers, BOOKMARK_SIMPLE_SVG, "Markers"),
        ];
        let bottom_tabs = [
            (ActivityRailTab::Settings, GEAR_SIX_SVG, "Settings"),
        ];

        let mut rail = div()
            .w(px(RAIL_WIDTH))
            .h_full()
            .flex()
            .flex_col()
            .justify_between()
            .items_center()
            .bg(ZED.surface_background)
            .border_r_1()
            .border_color(ZED.border_variant);

        // Top navigation tabs
        let mut top_container = div()
            .flex()
            .flex_col()
            .items_center()
            .gap_1()
            .pt_2()
            .w_full();

        for (tab, svg, _label) in top_tabs {
            top_container = top_container.child(Self::render_rail_button(tab, svg, sidebar, cx));
        }

        // Bottom navigation tabs
        let mut bottom_container = div()
            .flex()
            .flex_col()
            .items_center()
            .gap_1()
            .pb_2()
            .w_full();

        for (tab, svg, _label) in bottom_tabs {
            bottom_container = bottom_container.child(Self::render_rail_button(tab, svg, sidebar, cx));
        }

        rail = rail.child(top_container).child(bottom_container);
        rail.into_any_element()
    }

    fn render_rail_button(
        tab: ActivityRailTab,
        svg_data: &'static [u8],
        sidebar: &SidebarStateMachine,
        cx: &mut gpui::Context<crate::app::AppView>,
    ) -> impl IntoElement {
        let is_active = sidebar.active_tab == tab && sidebar.state == SidebarState::Expanded;
        let tab_clone = tab;

        div()
            .id(ElementId::from(format!("rail_{:?}", tab)))
            .cursor_pointer()
            .relative()
            .w(px(32.0))
            .h(px(32.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded_md()
            .bg(if is_active {
                ZED.ghost_selected
            } else {
                ZED.transparent
            })
            .hover(|s| s.bg(ZED.ghost_hover))
            // Vertical accent bar aligned flush to the left edge of the rail
            .when(is_active, |this| {
                this.child(
                    div()
                        .absolute()
                        .left(px(-4.0))
                        .top(px(4.0))
                        .bottom(px(4.0))
                        .w(px(2.5))
                        .rounded_r_sm()
                        .bg(ZED.icon_accent),
                )
            })
            .child(
                div()
                    .w(px(18.0))
                    .h(px(18.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(render_icon(
                        svg_data,
                        16.0,
                        if is_active {
                            ZED.icon_accent
                        } else {
                            ZED.icon_muted
                        },
                    )),
            )
            .on_click(cx.listener(move |this, _, _window, cx| {
                this.sidebar_state.activate_tab(tab_clone);
                cx.notify();
            }))
    }

    fn render_drawer(
        sidebar: &SidebarStateMachine,
        workspace: &WorkspaceState,
        expanded_projects: &HashSet<String>,
        all_entries: &[crate::model::LogEntry],
        visible_indices: &[usize],
        density: Density,
        tail_enabled: bool,
        cx: &mut gpui::Context<crate::app::AppView>,
    ) -> gpui::AnyElement {
        let header = Self::render_drawer_header(sidebar.active_tab, cx);

        let content = match sidebar.active_tab {
            ActivityRailTab::Files => {
                let active_file_id = workspace
                    .active_project()
                    .and_then(|p| p.working_set.active_tab())
                    .and_then(|tab| {
                        if let crate::workspace::working_set::TabTarget::LogFile { file_id } =
                            &tab.target
                        {
                            Some(file_id.as_str())
                        } else {
                            None
                        }
                    });

                WorkspaceTree::render(
                    workspace,
                    workspace.active_project_id.as_deref(),
                    active_file_id,
                    expanded_projects,
                    cx,
                )
            }
            ActivityRailTab::Investigations => {
                Self::render_investigations_panel(workspace, cx)
            }
            ActivityRailTab::SavedViews => {
                Self::render_saved_views_panel(workspace, cx)
            }
            ActivityRailTab::Markers => {
                Self::render_markers_panel(all_entries, visible_indices, cx)
            }
            ActivityRailTab::Settings => {
                Self::render_settings_panel(density, tail_enabled, cx)
            }
        };

        let view_drag_move = cx.entity().clone();
        let is_resizing = sidebar.is_resizing;

        let resize_handle = div()
            .id(ElementId::from("drawer_resize_handle"))
            .occlude()
            .cursor_col_resize()
            .absolute()
            .top_0()
            .bottom_0()
            .right_0()
            .w(px(5.0))
            .h_full()
            .hover(|s| s.bg(ZED.ghost_hover))
            .when(is_resizing, |s| s.bg(ZED.ghost_selected))
            .on_drag(ResizeDrawer, move |drag, _point, _window, cx| {
                cx.stop_propagation();
                cx.new(|_| drag.clone())
            })
            .on_drag_move(move |e: &gpui::DragMoveEvent<ResizeDrawer>, _window, cx| {
                view_drag_move.update(cx, |this, cx| {
                    this.update_drawer_resize(e.event.position.x, cx);
                });
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &gpui::MouseDownEvent, _window, cx| {
                    if event.click_count >= 2 {
                        this.reset_drawer_width(cx);
                    } else {
                        this.start_resizing_drawer(cx);
                    }
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.finish_resizing_drawer(cx);
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.finish_resizing_drawer(cx);
                }),
            )
            .on_click(move |_event, _window, cx| {
                cx.stop_propagation();
            });

        div()
            .relative()
            .w(px(sidebar.drawer_width))
            .h_full()
            .flex()
            .flex_col()
            .bg(ZED.surface_background)
            .border_r_1()
            .border_color(ZED.border_variant)
            .child(header)
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .overflow_hidden()
                    .child(content),
            )
            .child(resize_handle)
            .into_any_element()
    }

    fn render_drawer_header(
        tab: ActivityRailTab,
        cx: &mut gpui::Context<crate::app::AppView>,
    ) -> gpui::AnyElement {
        let (title, is_files, is_inv, is_views) = match tab {
            ActivityRailTab::Files => ("EXPLORER", true, false, false),
            ActivityRailTab::Investigations => ("INVESTIGATIONS", false, true, false),
            ActivityRailTab::SavedViews => ("SAVED VIEWS", false, false, true),
            ActivityRailTab::Markers => ("BOOKMARKS & MARKERS", false, false, false),
            ActivityRailTab::Settings => ("SETTINGS", false, false, false),
        };

        let mut header = div()
            .h(px(32.0))
            .w_full()
            .px_3()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(ZED.border_variant);

        let title_el = div()
            .text_xs()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(ZED.text_muted)
            .child(title);

        header = header.child(title_el);

        if is_files {
            let actions = div()
                .flex()
                .flex_row()
                .items_center()
                .gap_1()
                // Add log file button
                .child(
                    div()
                        .id(ElementId::from("drawer_hdr_add_file"))
                        .cursor_pointer()
                        .w(px(20.0))
                        .h(px(20.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_sm()
                        .hover(|s| s.bg(ZED.ghost_hover))
                        .child(render_icon(PLUS_SVG, 12.0, ZED.icon_muted))
                        .on_click(cx.listener(|this, _, _window, cx| {
                            this.open_file_dialog(cx);
                        })),
                )
                // Project Switcher button
                .child(
                    div()
                        .id(ElementId::from("drawer_hdr_project_switcher"))
                        .cursor_pointer()
                        .w(px(20.0))
                        .h(px(20.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_sm()
                        .hover(|s| s.bg(ZED.ghost_hover))
                        .child(render_icon(FOLDER_OPEN_SVG, 12.0, ZED.icon_muted))
                        .on_click(cx.listener(|this, _, _window, cx| {
                            this.toggle_project_switcher(cx);
                        })),
                )
                // Collapse all projects button
                .child(
                    div()
                        .id(ElementId::from("drawer_hdr_collapse_all"))
                        .cursor_pointer()
                        .w(px(20.0))
                        .h(px(20.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_sm()
                        .hover(|s| s.bg(ZED.ghost_hover))
                        .child(render_icon(ARROWS_IN_LINE_VERTICAL_SVG, 12.0, ZED.icon_muted))
                        .on_click(cx.listener(|this, _, _window, cx| {
                            this.collapse_all_projects(cx);
                        })),
                )
                // Expand all projects button
                .child(
                    div()
                        .id(ElementId::from("drawer_hdr_expand_all"))
                        .cursor_pointer()
                        .w(px(20.0))
                        .h(px(20.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_sm()
                        .hover(|s| s.bg(ZED.ghost_hover))
                        .child(render_icon(ARROW_COUNTER_CLOCKWISE_SVG, 12.0, ZED.icon_muted))
                        .on_click(cx.listener(|this, _, _window, cx| {
                            this.expand_all_projects(cx);
                        })),
                );

            header = header.child(actions);
        } else if is_inv {
            let actions = div()
                .flex()
                .flex_row()
                .items_center()
                .gap_1()
                .child(
                    div()
                        .id(ElementId::from("drawer_hdr_new_case"))
                        .cursor_pointer()
                        .w(px(20.0))
                        .h(px(20.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_sm()
                        .hover(|s| s.bg(ZED.ghost_hover))
                        .child(render_icon(PLUS_SVG, 12.0, ZED.icon_muted))
                        .on_click(cx.listener(|this, _, _window, cx| {
                            this.create_investigation_case(cx);
                        })),
                );
            header = header.child(actions);
        } else if is_views {
            let actions = div()
                .flex()
                .flex_row()
                .items_center()
                .gap_1()
                .child(
                    div()
                        .id(ElementId::from("drawer_hdr_save_view"))
                        .cursor_pointer()
                        .w(px(20.0))
                        .h(px(20.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_sm()
                        .hover(|s| s.bg(ZED.ghost_hover))
                        .child(render_icon(PLUS_SVG, 12.0, ZED.icon_muted))
                        .on_click(cx.listener(|this, _, _window, cx| {
                            this.save_current_view(cx);
                        })),
                );
            header = header.child(actions);
        }

        header.into_any_element()
    }

    fn render_investigations_panel(
        workspace: &WorkspaceState,
        cx: &mut gpui::Context<crate::app::AppView>,
    ) -> gpui::AnyElement {
        let active_proj = workspace.active_project();
        let empty_cases = Vec::new();
        let cases = active_proj.map(|p| &p.investigation_cases).unwrap_or(&empty_cases);

        let mut list = div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .p_2()
            .gap_1p5()
            .overflow_hidden();

        if cases.is_empty() {
            return div()
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
                        .text_xs()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(ZED.text)
                        .child("No Active Investigations"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(ZED.text_muted)
                        .child("Track incidents and gather cross-log evidence anchors."),
                )
                .child(
                    div()
                        .id(ElementId::from("empty_create_case"))
                        .cursor_pointer()
                        .px_2p5()
                        .py_1()
                        .rounded_sm()
                        .bg(ZED.element_background)
                        .border_1()
                        .border_color(ZED.border_variant)
                        .hover(|s| s.bg(ZED.element_hover))
                        .text_xs()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(ZED.text_accent)
                        .child("+ Create Incident Case")
                        .on_click(cx.listener(|this, _, _window, cx| {
                            this.create_investigation_case(cx);
                        })),
                )
                .into_any_element();
        }

        for case in cases {
            let case_id = case.id.clone();
            let anchor_count = case.anchors.len();
            let status = case.status;
            let (status_label, status_bg, status_fg) = match status {
                crate::workspace::investigation::CaseStatus::Active => ("ACTIVE", ZED.warning_bg, ZED.warning),
                crate::workspace::investigation::CaseStatus::RootCauseIdentified => ("ROOT CAUSE", ZED.info_bg, ZED.info),
                crate::workspace::investigation::CaseStatus::Mitigated => ("MITIGATED", ZED.success_bg, ZED.success),
                crate::workspace::investigation::CaseStatus::Archived => ("ARCHIVED", ZED.element_disabled, ZED.text_muted),
            };

            let row = div()
                .id(ElementId::from(format!("case_{}", case.id)))
                .cursor_pointer()
                .flex()
                .flex_col()
                .gap_1()
                .p_2()
                .rounded_md()
                .bg(ZED.element_background)
                .border_1()
                .border_color(ZED.border_variant)
                .hover(|s| s.bg(ZED.element_hover).border_color(ZED.border_focused))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_1p5()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(gpui::FontWeight::BOLD)
                                        .text_color(ZED.text_accent)
                                        .child(case.id.clone()),
                                )
                                .child(
                                    div()
                                        .px_1()
                                        .rounded_sm()
                                        .bg(ZED.error_bg)
                                        .text_color(ZED.error)
                                        .text_xs()
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .child(case.severity.clone()),
                                ),
                        )
                        .child(
                            div()
                                .px_1p5()
                                .py_0p5()
                                .rounded_sm()
                                .bg(status_bg)
                                .text_color(status_fg)
                                .text_xs()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child(status_label),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(ZED.text)
                        .child(case.title.clone()),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_1()
                        .text_xs()
                        .text_color(ZED.text_muted)
                        .child(render_icon(FLAG_SVG, 10.0, ZED.icon_muted))
                        .child(format!("{} evidence anchor{}", anchor_count, if anchor_count == 1 { "" } else { "s" })),
                )
                .on_click(cx.listener(move |this, _, _window, cx| {
                    this.open_investigation_case(&case_id, cx);
                }));

            list = list.child(row);
        }

        list.into_any_element()
    }

    fn render_saved_views_panel(
        workspace: &WorkspaceState,
        cx: &mut gpui::Context<crate::app::AppView>,
    ) -> gpui::AnyElement {
        let active_proj = workspace.active_project();
        let empty_views = Vec::new();
        let views = active_proj.map(|p| &p.saved_views).unwrap_or(&empty_views);

        let mut list = div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .p_2()
            .gap_1p5()
            .overflow_hidden();

        if views.is_empty() {
            return div()
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
                        .text_xs()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(ZED.text)
                        .child("No Saved Views"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(ZED.text_muted)
                        .child("Capture complex filter chains and column layouts for 1-click triage."),
                )
                .child(
                    div()
                        .id(ElementId::from("empty_save_view"))
                        .cursor_pointer()
                        .px_2p5()
                        .py_1()
                        .rounded_sm()
                        .bg(ZED.element_background)
                        .border_1()
                        .border_color(ZED.border_variant)
                        .hover(|s| s.bg(ZED.element_hover))
                        .text_xs()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(ZED.text_accent)
                        .child("+ Save Current Filters as View")
                        .on_click(cx.listener(|this, _, _window, cx| {
                            this.save_current_view(cx);
                        })),
                )
                .into_any_element();
        }

        for view in views {
            let view_id = view.id.clone();
            let filter_count = view.filter.conditions.len();
            let has_search = !view.search_query.trim().is_empty();

            let row = div()
                .id(ElementId::from(format!("view_{}", view.id)))
                .cursor_pointer()
                .flex()
                .flex_col()
                .gap_1()
                .p_2()
                .rounded_md()
                .bg(ZED.element_background)
                .border_1()
                .border_color(ZED.border_variant)
                .hover(|s| s.bg(ZED.element_hover).border_color(ZED.border_focused))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(ZED.text)
                                .child(view.name.clone()),
                        )
                        .child(
                            div()
                                .px_1()
                                .rounded_sm()
                                .bg(ZED.ghost_selected)
                                .text_color(ZED.text_accent)
                                .text_xs()
                                .child("APPLY"),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_1p5()
                        .text_xs()
                        .text_color(ZED.text_muted)
                        .child(format!("{} filter condition{}", filter_count, if filter_count == 1 { "" } else { "s" }))
                        .when(has_search, |this| {
                            this.child(div().child("•")).child(format!("Search: \"{}\"", view.search_query))
                        }),
                )
                .on_click(cx.listener(move |this, _, _window, cx| {
                    this.apply_saved_view(&view_id, cx);
                }));

            list = list.child(row);
        }

        list.into_any_element()
    }

    fn render_markers_panel(
        all_entries: &[crate::model::LogEntry],
        visible_indices: &[usize],
        cx: &mut gpui::Context<crate::app::AppView>,
    ) -> gpui::AnyElement {
        let mut list = div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .p_2()
            .gap_1()
            .overflow_hidden();

        let mut marker_count = 0usize;
        for (vis_pos, &entry_ix) in visible_indices.iter().enumerate() {
            if let Some(entry) = all_entries.get(entry_ix) {
                if let Some(marker) = entry.effective_marker() {
                    marker_count += 1;
                    let marker_sym = marker.symbol();
                    let marker_col = marker.color();
                    let target_pos = vis_pos;

                    let item = div()
                        .id(ElementId::from(format!("drawer_mark_{}", entry_ix)))
                        .cursor_pointer()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_1p5()
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .bg(ZED.element_background)
                        .border_1()
                        .border_color(ZED.border_variant)
                        .hover(|s| s.bg(ZED.element_hover))
                        .child(
                            div()
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_color(marker_col)
                                .child(marker_sym),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(ZED.text_muted)
                                .child(format!("#{}", entry.index)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_xs()
                                .text_color(ZED.text)
                                .truncate()
                                .child(entry.payload.clone()),
                        )
                        .on_click(cx.listener(move |this, _, _window, cx| {
                            this.jump_to_row(target_pos, cx);
                        }));

                    list = list.child(item);
                }
            }
        }

        if marker_count == 0 {
            return div()
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
                        .text_xs()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(ZED.text)
                        .child("No Bookmarks or Markers"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(ZED.text_muted)
                        .child("Bookmark lines or add markers to easily jump back to critical events."),
                )
                .into_any_element();
        }

        list.into_any_element()
    }

    fn render_settings_panel(
        density: Density,
        tail_enabled: bool,
        cx: &mut gpui::Context<crate::app::AppView>,
    ) -> gpui::AnyElement {
        let densities = [
            (Density::Comfortable, "Comfortable (30px)"),
            (Density::Compact, "Compact (24px)"),
            (Density::UltraCompact, "Ultra Compact (20px)"),
        ];

        let mut panel = div()
            .flex()
            .flex_col()
            .gap_3()
            .p_3()
            .w_full();

        // Density Section
        let mut density_sec = div().flex().flex_col().gap_1p5().w_full();
        density_sec = density_sec.child(
            div()
                .text_xs()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(ZED.text)
                .child("Row Density"),
        );

        for (d, label) in densities {
            let is_sel = density == d;
            let d_val = d;
            let btn = div()
                .id(ElementId::from(format!("settings_density_{:?}", d)))
                .cursor_pointer()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .px_2()
                .py_1()
                .rounded_sm()
                .bg(if is_sel {
                    ZED.element_selected
                } else {
                    ZED.element_background
                })
                .border_1()
                .border_color(if is_sel {
                    ZED.border_focused
                } else {
                    ZED.border_variant
                })
                .hover(|s| s.bg(ZED.element_hover))
                .child(
                    div()
                        .text_xs()
                        .text_color(if is_sel {
                            ZED.text_accent
                        } else {
                            ZED.text
                        })
                        .child(label),
                )
                .when(is_sel, |this| {
                    this.child(
                        div()
                            .w(px(5.0))
                            .h(px(5.0))
                            .rounded_full()
                            .bg(ZED.icon_accent),
                    )
                })
                .on_click(cx.listener(move |this, _, _window, cx| {
                    this.set_density(d_val, cx);
                }));

            density_sec = density_sec.child(btn);
        }

        panel = panel.child(density_sec);

        // Auto-Tail Follow Section
        let tail_btn = div()
            .id(ElementId::from("settings_tail_toggle"))
            .cursor_pointer()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px_2()
            .py_1p5()
            .rounded_sm()
            .bg(ZED.element_background)
            .border_1()
            .border_color(ZED.border_variant)
            .hover(|s| s.bg(ZED.element_hover))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(ZED.text)
                            .child("Auto-Tail Follow"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(ZED.text_muted)
                            .child("Follow newest log entries automatically"),
                    ),
            )
            .child(
                div()
                    .px_1p5()
                    .py_0p5()
                    .rounded_sm()
                    .bg(if tail_enabled {
                        ZED.success_bg
                    } else {
                        ZED.element_disabled
                    })
                    .text_color(if tail_enabled {
                        ZED.success
                    } else {
                        ZED.text_muted
                    })
                    .text_xs()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(if tail_enabled { "ON" } else { "OFF" }),
            )
            .on_click(cx.listener(|this, _, _window, cx| {
                this.toggle_tail(cx);
            }));

        panel = panel.child(tail_btn);

        // Keyboard shortcuts cheat sheet
        let shortcuts = [
            ("⌘B", "Toggle Sidebar"),
            ("⌘⇧B", "Explorer (Files)"),
            ("⌘⇧I", "Investigations"),
            ("⌘⇧S", "Saved Views"),
            ("⌘⇧M", "Markers"),
            ("⌘K", "Project Switcher"),
            ("⌘F", "Search Logs"),
            ("⌘1-9", "Switch Tab"),
        ];

        let mut sc_sec = div().flex().flex_col().gap_1().w_full().pt_2();
        sc_sec = sc_sec.child(
            div()
                .text_xs()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(ZED.text_muted)
                .child("SHORTCUTS"),
        );

        for (keys, label) in shortcuts {
            let row = div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .py_0p5()
                .child(
                    div()
                        .text_xs()
                        .text_color(ZED.text_muted)
                        .child(label),
                )
                .child(
                    div()
                        .px_1()
                        .rounded_sm()
                        .bg(ZED.element_background)
                        .border_1()
                        .border_color(ZED.border_variant)
                        .text_xs()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(ZED.text)
                        .child(keys),
                );

            sc_sec = sc_sec.child(row);
        }

        panel = panel.child(sc_sec);

        panel.into_any_element()
    }
}
