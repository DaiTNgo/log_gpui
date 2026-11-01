//! Right context sidebar for the focused log file: a line naming what it
//! acts on, the Filter / Highlights / Columns sections in a scrolling area,
//! and the Inspector pinned at the bottom under a splitter. Every section
//! reflects the state of the focused file only. The left edge sets the width.

use crate::app::AppView;
use crate::workspace::state::{ContextSection, ContextSidebarLayout, ContextSidebarState, MIN_CONTEXT_PART_HEIGHT};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    AppContext as _, Bounds, ElementId, InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels, Render, ScrollHandle,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_component::scroll::{Scrollbar, ScrollbarAxis};
use gpui_component::{ActiveTheme as _, Icon, h_flex, v_flex};
use gpui_kit::assets::IconName;
use std::cell::Cell;
use std::rc::Rc;

/// Window height taken by everything outside the sidebar (title bar, tabs,
/// toolbar, status bar): the sidebar's height before it is measured.
const WINDOW_CHROME_HEIGHT: f32 = 170.0;
/// Section headers keep this height with or without a summary.
const SECTION_HEADER_HEIGHT: f32 = 32.0;
/// Height of the line naming the file and the view.
const SCOPE_HEIGHT: f32 = 28.0;

/// Compact count badge. Fixed height, so it appearing never changes the row
/// it sits in (gpui-component's `Tag` is taller than the 12px text beside it).
pub fn count_badge(text: impl Into<gpui::SharedString>, cx: &gpui::App) -> gpui::Div {
    div()
        .flex_shrink_0()
        .h(px(16.0))
        .min_w(px(16.0))
        .px_1()
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(cx.theme().muted)
        .text_color(cx.theme().muted_foreground)
        .text_size(px(10.0))
        .child(text.into())
}

/// Dragging the splitter above the Inspector.
#[derive(Clone)]
pub struct ResizeInspector;

impl Render for ResizeInspector {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
    }
}

/// Dragging the left edge of the sidebar.
#[derive(Clone)]
pub struct ResizeContextSidebar;

impl Render for ResizeContextSidebar {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
    }
}

/// One section: its body, the summary shown on its header and the buttons
/// at the header's end.
pub struct SectionParts {
    pub body: gpui::AnyElement,
    pub summary: Option<String>,
    pub actions: Option<gpui::AnyElement>,
}

impl SectionParts {
    pub fn new(body: gpui::AnyElement) -> Self {
        Self { body, summary: None, actions: None }
    }
}

/// Pre-rendered sections.
pub struct ContextSections {
    /// What the sidebar acts on: the file or Log Space and the view.
    pub scope: String,
    pub filter: SectionParts,
    pub highlights: SectionParts,
    pub columns: SectionParts,
    pub inspector: SectionParts,
    /// Filter, Highlights and Columns only mean something for parsed rows;
    /// the Raw View shows the Inspector alone.
    pub parsed: bool,
}

pub struct ContextSidebar;

impl ContextSidebar {
    fn section_header(
        state: &ContextSidebarState,
        kind: ContextSection,
        summary: Option<String>,
        actions: Option<gpui::AnyElement>,
        cx: &mut gpui::Context<AppView>,
    ) -> gpui::AnyElement {
        let collapsed = state.is_collapsed(kind);
        h_flex()
            .id(ElementId::from(format!("ctx-section-{}", kind.as_str())))
            .w_full()
            .h(px(SECTION_HEADER_HEIGHT))
            .flex_shrink_0()
            .pl_3()
            .pr_2()
            .gap_2()
            .items_center()
            .cursor_pointer()
            .child(
                Icon::new(if collapsed { IconName::ChevronRight } else { IconName::ChevronDown })
                    .size_3()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(div().text_sm().font_weight(gpui::FontWeight::SEMIBOLD).child(kind.as_str()))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .truncate()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .children(summary),
            )
            .children(actions)
            .on_click(cx.listener(move |this, _, _window, cx| this.toggle_context_section(kind, cx)))
            .into_any_element()
    }

    fn section(
        state: &ContextSidebarState,
        kind: ContextSection,
        parts: SectionParts,
        cx: &mut gpui::Context<AppView>,
    ) -> gpui::AnyElement {
        let collapsed = state.is_collapsed(kind);
        let SectionParts { body, summary, actions } = parts;
        v_flex()
            .w_full()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(Self::section_header(state, kind, summary, actions, cx))
            .when(!collapsed, |this| this.child(div().w_full().px_3().pb_3().child(body)))
            .into_any_element()
    }

    pub fn render(
        state: &ContextSidebarState,
        layout: &ContextSidebarLayout,
        // Where the sidebar's body (under the scope line) was last drawn.
        body_bounds: Rc<Cell<Bounds<Pixels>>>,
        window: &mut Window,
        // Window height left beside the sidebar: the Focus Pane is under it.
        viewport_height: f32,
        sections: ContextSections,
        cx: &mut gpui::Context<AppView>,
    ) -> gpui::AnyElement {
        let ContextSections { scope, filter, highlights, columns, inspector, parsed } = sections;
        let theme_border = cx.theme().border;

        let measured = f32::from(body_bounds.get().size.height);
        let total = if measured > 0.0 { measured } else { (viewport_height - WINDOW_CHROME_HEIGHT).max(0.0) };
        let inspector_collapsed = state.is_collapsed(ContextSection::Inspector);
        let SectionParts { body: inspector_body, summary: inspector_summary, actions: inspector_actions } = inspector;
        let inspector_header =
            Self::section_header(state, ContextSection::Inspector, inspector_summary, inspector_actions, cx);
        let inspector_el = v_flex()
            .w_full()
            .min_h(px(0.0))
            .child(inspector_header)
            .when(!inspector_collapsed, |this| this.child(div().flex_1().min_h(px(0.0)).child(inspector_body)));

        let body = if parsed {
            let handle = window
                .use_keyed_state("ctx-sidebar-scroll", cx, |_, _| ScrollHandle::default())
                .read(cx)
                .clone();
            let scroll = div()
                .flex_1()
                .min_h(px(MIN_CONTEXT_PART_HEIGHT.min(total / 2.0)))
                .relative()
                .child(
                    div().id("context-sidebar").size_full().overflow_y_scroll().track_scroll(&handle).child(
                        v_flex()
                            .w_full()
                            .child(Self::section(state, ContextSection::Filter, filter, cx))
                            .child(Self::section(state, ContextSection::Highlights, highlights, cx))
                            .child(Self::section(state, ContextSection::Columns, columns, cx)),
                    ),
                )
                .child(
                    div().absolute().inset_0().child(
                        Scrollbar::new(&handle)
                            .id("ctx-sidebar-scrollbar")
                            .axis(ScrollbarAxis::Vertical)
                            .viewport_from_layout(),
                    ),
                );
            let view_move = cx.entity();
            let splitter = div()
                .id("ctx-inspector-splitter")
                .debug_selector(|| "ctx-inspector-splitter".into())
                .w_full()
                .h(px(5.0))
                .flex_shrink_0()
                .cursor_row_resize()
                .border_t_1()
                .border_color(theme_border)
                .on_drag(ResizeInspector, |drag, _, _, cx| {
                    cx.stop_propagation();
                    cx.new(|_| drag.clone())
                })
                .on_drag_move(move |e: &gpui::DragMoveEvent<ResizeInspector>, _, cx| {
                    view_move.update(cx, |this, cx| this.drag_inspector_split(e.event.position.y, cx));
                });
            let inspector_el = if inspector_collapsed {
                inspector_el.flex_shrink_0()
            } else {
                inspector_el.h(px(layout.inspector_height(total))).flex_shrink_0()
            };
            v_flex()
                .size_full()
                .child(scroll)
                .when(!inspector_collapsed, |this| this.child(splitter))
                .when(inspector_collapsed, |this| this.child(div().border_t_1().border_color(theme_border)))
                .child(inspector_el)
                .into_any_element()
        } else {
            v_flex().size_full().child(inspector_el.flex_1()).into_any_element()
        };

        let view_width = cx.entity();
        let edge = div()
            .id("ctx-sidebar-edge")
            .debug_selector(|| "ctx-sidebar-edge".into())
            .occlude()
            .absolute()
            .top_0()
            .bottom_0()
            .left(px(-2.0))
            .w(px(5.0))
            .cursor_col_resize()
            .on_drag(ResizeContextSidebar, |drag, _, _, cx| {
                cx.stop_propagation();
                cx.new(|_| drag.clone())
            })
            .on_drag_move(move |e: &gpui::DragMoveEvent<ResizeContextSidebar>, _, cx| {
                view_width.update(cx, |this, cx| this.drag_context_width(e.event.position.x, cx));
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                    if event.click_count >= 2 {
                        this.reset_context_width(cx);
                    }
                }),
            );

        div()
            .relative()
            .w(px(layout.width))
            .h_full()
            .flex_shrink_0()
            .border_l_1()
            .border_color(theme_border)
            .bg(cx.theme().background)
            .child(
                v_flex()
                    .size_full()
                    .child(
                        h_flex()
                            .id("ctx-scope")
                            .h(px(SCOPE_HEIGHT))
                            .flex_shrink_0()
                            .px_3()
                            .items_center()
                            .border_b_1()
                            .border_color(theme_border)
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(div().truncate().child(scope)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_h(px(0.0))
                            .relative()
                            .child(
                                gpui::canvas(|bounds, _, _| bounds, move |bounds, _, _, _| body_bounds.set(bounds))
                                    .absolute()
                                    .size_full(),
                            )
                            .child(body),
                    ),
            )
            .child(edge)
            .into_any_element()
    }
}
