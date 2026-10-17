//! UI: editor toolbar built from gpui-kit components (Editor, Button, Tag).

use super::AppView;
use crate::ui::floating_panel::PanelKind;
use gpui::prelude::FluentBuilder as _;
use gpui::{div, px, Context, IntoElement, ParentElement, SharedString, Styled};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::Editor;
use gpui_component::tag::Tag;
use gpui_component::{h_flex, ActiveTheme as _, Icon, Selectable as _, Sizable as _};
use gpui_kit::assets::IconName;

impl AppView {
    /// Editor toolbar: query search field, active filter chips and panel toggles.
    pub(super) fn render_editor_toolbar(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let mut filter_chips = h_flex().gap_1();
        for cond in &self.filter_engine.conditions {
            if !cond.enabled || cond.value.trim().is_empty() {
                continue;
            }
            let cond_id = cond.id;
            filter_chips = filter_chips.child(
                Button::new(("filter-chip", cond_id))
                    .outline()
                    .xsmall()
                    .label(cond.to_chip_label())
                    .icon(IconName::Close)
                    .on_click(cx.listener(move |this, _, _window, cx| {
                        this.remove_filter_condition(cond_id, cx);
                    })),
            );
        }

        let parse_error = self.search_state.parse_error.as_ref().map(|e| {
            let msg = e.message();
            if msg.chars().count() > 48 {
                format!("! {}", msg.chars().take(48).collect::<String>())
            } else {
                format!("! {}", msg)
            }
        });

        let (match_cur, match_total) = self.search_state.current_match_display();

        let panel_toggle = |id: &'static str, label: &'static str, kind: PanelKind, cx: &mut Context<Self>| {
            Button::new(id)
                .ghost()
                .xsmall()
                .label(label)
                .selected(self.panel(kind).open)
                .on_click(cx.listener(move |this, _, _window, cx| this.toggle_panel(kind, cx)))
        };

        h_flex()
            .w_full()
            .px_3()
            .py_1()
            .gap_2()
            .items_center()
            // Query editor (query-LSP completions) in a bordered search box
            .child(
                h_flex()
                    .flex_1()
                    .min_w(px(0.0))
                    .gap_3()
                    .items_center()
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w(px(0.0))
                            .h(px(32.0))
                            .px_2()
                            .gap_2()
                            .items_center()
                            .rounded_md()
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().background)
                            .child(
                                Icon::new(IconName::Search)
                                    .size_4()
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .child(Editor::new(&self.search_input).appearance(false).h(px(28.0))),
                            )
                            .when(
                                self.search_state.is_open && !self.search_state.query.is_empty(),
                                |this| {
                                    this.child(
                                        Button::new("search-clear")
                                            .ghost()
                                            .xsmall()
                                            .icon(IconName::Close)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.search_input.update(cx, |state, cx| {
                                                    state.replace_all(SharedString::from(""), window, cx);
                                                });
                                                this.run_search(cx);
                                                this.search_input
                                                    .update(cx, |state, cx| state.focus(window, cx));
                                            })),
                                    )
                                },
                            )
                            .when_some(parse_error, |this, shown| this.child(Tag::danger().child(shown))),
                    )
                    // Match navigator: prev / next / current-of-total
                    .child(
                        h_flex()
                            .gap_1()
                            .items_center()
                            .text_color(cx.theme().muted_foreground)
                            .child(
                                Button::new("match-prev")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::ChevronLeft)
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        this.jump_prev_match(cx);
                                    })),
                            )
                            .child(
                                Button::new("match-next")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::ChevronRight)
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        this.jump_next_match(cx);
                                    })),
                            )
                            .child(
                                div()
                                    .min_w(px(48.0))
                                    .text_sm()
                                    .child(format!("{}/{}", match_cur, match_total)),
                            ),
                    ),
            )
            .child(filter_chips)
            .child(
                h_flex()
                    .gap_1()
                    .child(panel_toggle("toolbar-view", "View", PanelKind::View, cx))
                    .child(panel_toggle("toolbar-filter", "Filter", PanelKind::Filter, cx))
                    .child(panel_toggle("toolbar-marker", "Marker", PanelKind::Marker, cx))
                    .child(
                        Button::new("toolbar-inspector")
                            .ghost()
                            .xsmall()
                            .label("Inspector")
                            .selected(self.is_inspector_open)
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.is_inspector_open = !this.is_inspector_open;
                                cx.notify();
                            })),
                    ),
            )
            .into_any_element()
    }
}
