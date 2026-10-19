//! UI: editor toolbar built from gpui-kit components (Input, Button, Tag).

use super::AppView;
use gpui::prelude::FluentBuilder as _;
use gpui::{div, px, Context, IntoElement, ParentElement, SharedString, Styled};
use crate::workspace::working_set::ViewMode;
use gpui_component::button::{Button, ButtonGroup, ButtonVariants as _};
use gpui_component::input::Input;
use gpui_component::tag::Tag;
use gpui_component::{h_flex, ActiveTheme as _, Icon, Selectable as _, Sizable as _};
use gpui_kit::assets::IconName;

impl AppView {
    /// Editor toolbar: search field and the view switch.
    pub(super) fn render_editor_toolbar(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let raw_mode = self.view_mode == ViewMode::Raw;
        let is_log = self.active_tab_is_log_file();
        let parse_error = self.focus.search().parse_error.as_ref().map(|e| {
            let msg = e.message();
            if msg.chars().count() > 48 {
                format!("! {}", msg.chars().take(48).collect::<String>())
            } else {
                format!("! {}", msg)
            }
        });

        let match_label = self.match_counter_label();

        h_flex()
            .w_full()
            .px_3()
            .py_1()
            .gap_2()
            .items_center()
            // Search box: text-only, `&&` / `||` / `()` combine phrases
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
                                    .child(
                                        Input::new(&self.search_input)
                                            .appearance(false)
                                            .bordered(false)
                                            .focus_bordered(false)
                                            .h(px(28.0)),
                                    ),
                            )
                            .when(
                                self.focus.search().is_open && !self.focus.search().query.is_empty(),
                                |this| {
                                    this.child(
                                        Button::new("search-clear")
                                            .ghost()
                                            .xsmall()
                                            .icon(IconName::Close)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.search_input.update(cx, |state, cx| {
                                                    state.set_value(SharedString::from(""), window, cx);
                                                });
                                                // set_value does not emit a Change event, so
                                                // the query is cleared here.
                                                this.focus.search_mut().query.clear();
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
                                    .child(match_label),
                            ),
                    ),
            )
            .when(is_log, |this| {
                this.child(
                    ButtonGroup::new("view-mode")
                        .small()
                        .child(
                            Button::new("view-mode-raw")
                                .label("Raw")
                                .selected(raw_mode),
                        )
                        .child(
                            Button::new("view-mode-parsed")
                                .label("Parsed")
                                .selected(!raw_mode),
                        )
                        .on_click(cx.listener(|this, ix: &Vec<usize>, _, cx| {
                            if let Some(&i) = ix.first() {
                                this.set_view_mode(
                                    if i == 0 { ViewMode::Raw } else { ViewMode::Parsed },
                                    cx,
                                );
                            }
                        })),
                )
            })
            .into_any_element()
    }
}
