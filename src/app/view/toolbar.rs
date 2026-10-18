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
    /// Editor toolbar: search field, and active filter chips.
    pub(super) fn render_editor_toolbar(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let raw_mode = self.view_mode == ViewMode::Raw;
        let is_log = self.active_tab_is_log_file();
        let mut filter_chips = h_flex().gap_1();
        for (ix, cf) in self.filter_state.columns.iter().enumerate() {
            if raw_mode || !cf.is_active() {
                continue;
            }
            let label = self
                .filter_index
                .column(&cf.column)
                .map_or_else(|| cf.column.clone(), |c| c.label.clone());
            let mut parts: Vec<String> = Vec::new();
            for folded in &cf.values {
                let shown = self
                    .filter_index
                    .column(&cf.column)
                    .and_then(|c| c.values.iter().find(|v| &v.folded == folded))
                    .map_or_else(|| folded.clone(), |v| v.display.clone());
                parts.push(shown);
            }
            for text in &cf.custom {
                parts.push(format!("“{}”", text));
            }
            let chip = format!("{} {} {}", label, if cf.hide { "≠" } else { ":" }, parts.join(", "));
            let key = cf.column.clone();
            filter_chips = filter_chips.child(
                Button::new(("filter-chip", ix))
                    .outline()
                    .xsmall()
                    .label(chip)
                    .icon(IconName::Close)
                    .on_click(cx.listener(move |this, _, _window, cx| {
                        this.filter_clear_column(&key, cx);
                    })),
            );
        }
        if !raw_mode && self.filter_state.time_range.is_active() {
            let tr = &self.filter_state.time_range;
            let chip = format!("Time {} – {}", tr.from, tr.to);
            filter_chips = filter_chips.child(
                Button::new("filter-chip-time")
                    .outline()
                    .xsmall()
                    .label(chip)
                    .icon(IconName::Close)
                    .on_click(cx.listener(|this, _, _window, cx| {
                        this.filter_state.set_time_range("", "");
                        this.filter_ui.inputs_dirty = true;
                        this.apply_filters_and_refresh(cx);
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

        let (match_cur, match_total) = if raw_mode {
            self.raw.match_display()
        } else {
            self.search_state.current_match_display()
        };

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
                                self.search_state.is_open && !self.search_state.query.is_empty(),
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
                .child(
                    Button::new("quick-settings")
                        .ghost()
                        .small()
                        .icon(IconName::Settings2)
                        .selected(self.quick_settings_open)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.quick_settings_open = !this.quick_settings_open;
                            cx.notify();
                        })),
                )
            })
            .into_any_element()
    }
}
