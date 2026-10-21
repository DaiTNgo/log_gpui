//! Right context sidebar for the focused log file: four stacked, collapsible
//! sections (Filter / Highlights / Columns / Inspector). Every section reflects the
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

/// Window height taken by everything outside the sidebar (title bar, tabs, toolbar, status bar).
const WINDOW_CHROME_HEIGHT: f32 = 170.0;
/// The Inspector never shrinks below this.
const INSPECTOR_MIN_HEIGHT: f32 = 240.0;

/// Pre-rendered section bodies plus the badge counts shown in the headers.
pub struct ContextSections {
    pub filter: gpui::AnyElement,
    pub highlights: gpui::AnyElement,
    pub columns: gpui::AnyElement,
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
            columns,
            inspector,
            active_filters,
            rule_count,
            parsed,
        } = sections;

        let section = |kind: ContextSection,
                       badge: Option<usize>,
                       body: gpui::AnyElement,
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
                        this.child(div().w_full().px_3().pb_3().child(body))
                    }
                })
        };

        let handle = window
            .use_keyed_state("ctx-sidebar-scroll", cx, |_, _| ScrollHandle::default())
            .read(cx)
            .clone();
        let min_height = (viewport_height - WINDOW_CHROME_HEIGHT).max(INSPECTOR_MIN_HEIGHT);

        // The whole sidebar scrolls; sections grow with their content and the
        // Inspector soaks up whatever height is left over.
        div()
            .relative()
            .w(px(CONTEXT_SIDEBAR_WIDTH))
            .h_full()
            .flex_shrink_0()
            .border_l_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .child(
                div()
                    .id("context-sidebar")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&handle)
                    .child(
                        div()
                            .w_full()
                            .min_h(px(min_height))
                            .flex()
                            .flex_col()
                            .when(parsed, |this| {
                                this.child(section(ContextSection::Filter, Some(active_filters), filter, cx))
                                    .child(section(ContextSection::Highlights, Some(rule_count), highlights, cx))
                                    .child(section(ContextSection::Columns, None, columns, cx))
                            })
                            .child(section(ContextSection::Inspector, None, inspector, cx)),
                    ),
            )
            .child(
                div().absolute().inset_0().child(
                    Scrollbar::new(&handle)
                        .id("ctx-sidebar-scrollbar")
                        .axis(ScrollbarAxis::Vertical)
                        .viewport_from_layout(),
                ),
            )
            .into_any_element()
    }
}
