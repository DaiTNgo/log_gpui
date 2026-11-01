//! UI: the Filter Bar above the Parsed View's table. One chip per condition
//! of the Filter, Clear, and a button pausing the Filter. It shows while the
//! Filter has a condition, the context sidebar open or not.

use super::AppView;
use crate::workspace::state::ContextSection;
use gpui::prelude::FluentBuilder as _;
use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, SharedString, StatefulInteractiveElement, Styled, div,
    px,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, h_flex};
use gpui_kit::assets::IconName;

impl AppView {
    /// Pause the Filter (every row shows, the conditions stay) or apply it again.
    pub fn toggle_filter_paused(&mut self, cx: &mut Context<Self>) {
        self.filter_state.paused = !self.filter_state.paused;
        self.apply_filter_change(cx);
    }

    /// Open the Filter section with the card of `key` open.
    pub fn reveal_filter_column(&mut self, key: &str, cx: &mut Context<Self>) {
        self.filter_ui.expanded = Some(key.to_string());
        self.filter_ui.picker_open = false;
        self.reveal_context_section(ContextSection::Filter, cx);
    }

    pub fn clear_time_range(&mut self, cx: &mut Context<Self>) {
        self.filter_state.set_time_range("", "");
        self.filter_ui.inputs_dirty = true;
        self.apply_filter_change(cx);
    }

    /// The label of a column, as the Filter section names it.
    fn filter_column_label(&self, key: &str) -> String {
        match self.filter_index.column(key) {
            Some(c) => c.label.clone(),
            None => crate::columns::default_name(key).to_string(),
        }
    }

    /// A Filter Value as the column shows it.
    fn filter_value_display(&self, key: &str, folded: &str) -> String {
        self.filter_index
            .column(key)
            .and_then(|c| c.values.iter().find(|v| v.folded == folded))
            .map_or_else(|| folded.to_string(), |v| v.display.clone())
    }

    fn filter_chip(
        &self,
        id: String,
        text: String,
        on_open: impl Fn(&mut AppView, &mut Context<AppView>) + 'static,
        on_remove: impl Fn(&mut AppView, &mut Context<AppView>) + 'static,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let theme = cx.theme();
        let paused = self.filter_state.paused;
        h_flex()
            .id(SharedString::from(id.clone()))
            .h(px(22.0))
            .pl_2()
            .pr_0p5()
            .gap_0p5()
            .rounded(px(11.0))
            .border_1()
            .border_color(theme.border)
            .bg(theme.secondary)
            .text_xs()
            .when(paused, |this| this.line_through().text_color(theme.muted_foreground))
            .cursor_pointer()
            .hover(|this| this.bg(theme.secondary_hover))
            .on_click(cx.listener(move |this, _, _, cx| on_open(this, cx)))
            .child({
                let selector = id.clone();
                div().max_w(px(260.0)).truncate().debug_selector(move || selector.clone()).child(text)
            })
            .child({
                let selector = format!("{id}-remove");
                div().debug_selector(move || selector.clone()).child(
                    Button::new(SharedString::from(format!("{id}-remove")))
                        .ghost()
                        .xsmall()
                        .icon(IconName::Close)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            on_remove(this, cx)
                        })),
                )
            })
            .into_any_element()
    }

    /// The Filter Bar, or `None` while the Filter is empty.
    pub(super) fn render_filter_bar(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        if self.filter_state.is_empty() {
            return None;
        }
        let paused = self.filter_state.paused;
        let mut chips = h_flex().flex_1().min_w(px(0.0)).flex_wrap().gap_1();
        for filter in self.filter_state.columns.iter().filter(|c| c.is_active()) {
            let key = filter.column.clone();
            let text = filter.chip_text(&self.filter_column_label(&key), |v| self.filter_value_display(&key, v));
            let (open, remove) = (key.clone(), key.clone());
            chips = chips.child(self.filter_chip(
                format!("filter-chip-{key}"),
                text,
                move |this, cx| this.reveal_filter_column(&open, cx),
                move |this, cx| this.filter_clear_column(&remove, cx),
                cx,
            ));
        }
        let range = &self.filter_state.time_range;
        if range.is_active() {
            let text = match (range.from.is_empty(), range.to.is_empty()) {
                (false, false) => format!("time: {} – {}", range.from, range.to),
                (false, true) => format!("time: from {}", range.from),
                _ => format!("time: until {}", range.to),
            };
            chips = chips.child(self.filter_chip(
                "filter-chip-time".to_string(),
                text,
                |this, cx| this.reveal_context_section(ContextSection::Filter, cx),
                |this, cx| this.clear_time_range(cx),
                cx,
            ));
        }
        let theme = cx.theme();
        Some(
            h_flex()
                .id("filter-bar")
                .w_full()
                .px_2()
                .py_1()
                .gap_2()
                .items_center()
                .border_b_1()
                .border_color(theme.border)
                .child(Icon::new(IconName::Funnel).xsmall().text_color(theme.muted_foreground))
                .child(chips)
                .child(
                    Button::new("filter-bar-pause")
                        .ghost()
                        .xsmall()
                        .icon(if paused { IconName::Play } else { IconName::Pause })
                        .tooltip(if paused { "Apply the Filter again" } else { "Pause the Filter: show every line" })
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_filter_paused(cx))),
                )
                .child(
                    Button::new("filter-bar-clear")
                        .ghost()
                        .xsmall()
                        .label("Clear")
                        .on_click(cx.listener(|this, _, _, cx| this.filter_clear_all(cx))),
                )
                .into_any_element(),
        )
    }
}
