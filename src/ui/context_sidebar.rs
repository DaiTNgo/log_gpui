//! Right context sidebar for the focused log file: four stacked, collapsible
//! sections (Filter / Highlights / Inspector). Every section reflects the
//! state of the focused file only; the Inspector takes the remaining height.

use crate::app::AppView;
use crate::workspace::state::{ContextSection, ContextSidebarState, CONTEXT_SIDEBAR_WIDTH};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    div, px, ElementId, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    ScrollHandle, Styled,
};
use gpui_component::scroll::{Scrollbar, ScrollbarAxis};
use gpui_component::tag::Tag;
use gpui_component::{h_flex, ActiveTheme as _, Icon};
use gpui_kit::assets::IconName;

/// Max height of a content-sized section before it scrolls internally.
const SECTION_MAX_HEIGHT: f32 = 260.0;
/// The Inspector never shrinks below this; the other sections give up room first.
/// Window height taken by everything outside the sidebar (title bar, tabs, toolbar, status bar).
const WINDOW_CHROME_HEIGHT: f32 = 170.0;
/// Height of a collapsed section header.
const SECTION_HEADER_HEIGHT: f32 = 40.0;
const SECTION_MIN_HEIGHT: f32 = 80.0;
const INSPECTOR_MIN_HEIGHT: f32 = 240.0;

/// Pre-rendered section bodies plus the badge counts shown in the headers.
pub struct ContextSections {
    pub filter: gpui::AnyElement,
    pub highlights: gpui::AnyElement,
    pub inspector: gpui::AnyElement,
    pub active_filters: usize,
    pub rule_count: usize,
    /// Filter and Highlights only mean something for parsed rows; the Raw
    /// View shows the Inspector alone.
    pub parsed: bool,
}

pub struct ContextSidebar;

impl ContextSidebar {
    pub fn render(
        state: &ContextSidebarState,
        window: &mut gpui::Window,
        // Window height left beside the sidebar: the Focus Pane is under it.
        viewport_height: f32,
        sections: ContextSections,
        cx: &mut gpui::Context<AppView>,
    ) -> gpui::AnyElement {
        let ContextSections {
            filter,
            highlights,
            inspector,
            active_filters,
            rule_count,
            parsed,
        } = sections;

        // Split the height left over by the Inspector between the open tool sections.
        let tools = [ContextSection::Filter, ContextSection::Highlights];
        let open_tools = tools.iter().filter(|t| !state.is_collapsed(**t)).count();
        let inspector_room = if state.is_collapsed(ContextSection::Inspector) {
            0.0
        } else {
            INSPECTOR_MIN_HEIGHT
        };
        let budget = viewport_height
            - WINDOW_CHROME_HEIGHT
            - SECTION_HEADER_HEIGHT * 3.0
            - inspector_room;
        let section_cap = if open_tools == 0 {
            SECTION_MAX_HEIGHT
        } else {
            (budget / open_tools as f32).clamp(SECTION_MIN_HEIGHT, SECTION_MAX_HEIGHT)
        };

        let section = |kind: ContextSection,
                       badge: Option<usize>,
                       body: gpui::AnyElement,
                       window: &mut gpui::Window,
                       cx: &mut gpui::Context<AppView>| {
            let collapsed = state.is_collapsed(kind);
            let fills = kind == ContextSection::Inspector;
            let header = h_flex()
                .id(ElementId::from(format!("ctx-section-{}", kind.as_str())))
                .w_full()
                .px_3()
                .py_2()
                .gap_2()
                .items_center()
                .cursor_pointer()
                .child(
                    Icon::new(if collapsed {
                        IconName::ChevronRight
                    } else {
                        IconName::ChevronDown
                    })
                    .size_3()
                    .text_color(cx.theme().muted_foreground),
                )
                .child(
                    div()
                        .flex_1()
                        .text_sm()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(kind.as_str()),
                )
                .when_some(badge.filter(|n| *n > 0), |this, n| {
                    this.child(Tag::secondary().child(format!("{}", n)))
                })
                .on_click(cx.listener(move |this, _, _window, cx| {
                    this.toggle_context_section(kind, cx);
                }));

            div()
                .w_full()
                .flex()
                .flex_col()
                .border_b_1()
                .border_color(cx.theme().border)
                .when(fills && !collapsed, |this| {
                    this.flex_1().min_h(px(INSPECTOR_MIN_HEIGHT))
                })
                .child(header)
                .when(!collapsed, |this| {
                    if fills {
                        this.child(body)
                    } else {
                        let handle = window
                            .use_keyed_state(
                                ElementId::from(format!("ctx-scroll-{}", kind.as_str())),
                                cx,
                                |_, _| ScrollHandle::default(),
                            )
                            .read(cx)
                            .clone();
                        // Plain `overflow_y_scroll` + an overlaid scrollbar: the scrollbar
                        // wrapper sizes its area to 100% of an auto-height parent, which
                        // defeats `max_h`.
                        this.child(
                            div()
                                .relative()
                                .w_full()
                                .child(
                                    div()
                                        .id(ElementId::from(format!("ctx-body-{}", kind.as_str())))
                                        .w_full()
                                        .max_h(px(section_cap))
                                        .overflow_y_scroll()
                                        .track_scroll(&handle)
                                        .px_3()
                                        .pb_3()
                                        .child(body),
                                )
                                .child(
                                    div().absolute().inset_0().child(
                                        Scrollbar::new(&handle)
                                            .id(ElementId::from(format!(
                                                "ctx-scrollbar-{}",
                                                kind.as_str()
                                            )))
                                            .axis(ScrollbarAxis::Vertical)
                                            .viewport_from_layout(),
                                    ),
                                ),
                        )
                    }
                })
        };

        div()
            .id("context-sidebar")
            .w(px(CONTEXT_SIDEBAR_WIDTH))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .border_l_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .when(parsed, |this| {
                this.child(section(ContextSection::Filter, Some(active_filters), filter, window, cx))
                    .child(section(ContextSection::Highlights, Some(rule_count), highlights, window, cx))
            })
            .child(section(ContextSection::Inspector, None, inspector, window, cx))
            .into_any_element()
    }
}
