//! UI: editor toolbar built from gpui-kit components (Input, Button, Tag).

use super::AppView;
use gpui::prelude::FluentBuilder as _;
use gpui::{div, px, Context, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement, SharedString, StatefulInteractiveElement as _, Styled};
use crate::workspace::working_set::ViewMode;
use gpui_component::button::{Button, ButtonGroup, ButtonVariants as _};
use gpui_component::input::Input;
use gpui_component::popover::Popover;
use gpui_component::tag::Tag;
use gpui_component::{h_flex, ActiveTheme as _, Icon, Selectable as _, Sizable as _};
use gpui_kit::assets::IconName;

impl AppView {
    /// Editor toolbar: search field and the view switch.
    pub(super) fn render_editor_toolbar(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let raw_mode = self.view_mode == ViewMode::Raw;
        let is_log = self.active_tab_is_log_file();
        // Collapse or expand every File Section, when the Raw View has several.
        let fold_all = (is_log && raw_mode && self.raw_sections().len() > 1).then(|| self.all_sections_collapsed());
        let parse_error = self.focus.search().parse_error.as_ref().map(|e| {
            let msg = e.message();
            if msg.chars().count() > 48 {
                format!("! {}", msg.chars().take(48).collect::<String>())
            } else {
                format!("! {}", msg)
            }
        });

        let match_label = self.match_counter_label();
        let (colors_under, colors_over) = crate::ui::phrase_chips::toolbar_search_layers(self, cx).unzip();

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
                            // ↑/↓ in an empty field step through the Recent Search
                            // Expressions; Escape goes back to what was typed.
                            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                                let m = event.keystroke.modifiers;
                                if m.platform || m.control || m.alt || m.shift {
                                    return;
                                }
                                let taken = match event.keystroke.key.as_str() {
                                    "up" => this.step_search_history(true, window, cx),
                                    "down" => this.step_search_history(false, window, cx),
                                    "escape" => this.leave_search_history(window, cx),
                                    _ => false,
                                };
                                if taken {
                                    cx.stop_propagation();
                                }
                            }))
                            .child(
                                Icon::new(IconName::Search)
                                    .size_4()
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .relative()
                                    .children(colors_under)
                                    .child(
                                        Input::new(&self.search_input)
                                            .appearance(false)
                                            .bordered(false)
                                            .focus_bordered(false)
                                            .h(px(28.0)),
                                    )
                                    .children(colors_over),
                            )
                            .when(!self.search_history().is_empty(), |this| {
                                this.child(self.render_search_history_button(cx))
                            })
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
                                // Room for "99999/99999" so the buttons don't shift as counts grow
                                div()
                                    .flex_none()
                                    .min_w(px(88.0))
                                    .whitespace_nowrap()
                                    .text_sm()
                                    .child(match_label),
                            ),
                    ),
            )
            .when_some(fold_all, |this, all_collapsed| {
                let (icon, tooltip) = if all_collapsed {
                    (IconName::ChevronsUpDown, "Expand All Files")
                } else {
                    (IconName::ChevronsDownUp, "Collapse All Files")
                };
                this.child(
                    Button::new("raw-fold-all")
                        .ghost()
                        .small()
                        .icon(icon)
                        .tooltip(tooltip)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.set_all_sections_collapsed(!all_collapsed, cx);
                        })),
                )
            })
            // What shows around the list: the Time Histogram and the
            // Swimlanes above it, the minimap beside it. Remembered for every view.
            .when(is_log && !raw_mode, |this| {
                let (histogram, lanes, minimap) =
                    (self.workspace_state.show_histogram, self.workspace_state.show_lanes, self.show_minimap);
                this.child(
                    h_flex()
                        .gap_0p5()
                        .child(
                            Button::new("show-histogram")
                                .ghost()
                                .small()
                                .label("Histogram")
                                .selected(histogram)
                                .tooltip("Show time histogram")
                                .on_click(cx.listener(move |this, _, _, cx| this.set_show_histogram(!histogram, cx))),
                        )
                        .child(
                            Button::new("show-lanes")
                                .ghost()
                                .small()
                                .label("Lanes")
                                .selected(lanes)
                                .tooltip("Show swimlanes")
                                .on_click(cx.listener(move |this, _, _, cx| this.set_show_lanes(!lanes, cx))),
                        )
                        .child(
                            Button::new("show-minimap")
                                .ghost()
                                .small()
                                .label("Minimap")
                                .selected(minimap)
                                .tooltip("Show minimap")
                                .on_click(cx.listener(move |this, _, _, cx| this.set_show_minimap(!minimap, cx))),
                        ),
                )
            })
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

impl AppView {
    /// The clock button beside the Search field listing the Recent Search
    /// Expressions: click one to search for it, × to forget it.
    fn render_search_history_button(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let view = cx.entity();
        let items = self.search_history();
        Popover::new("search-history")
            .trigger(Button::new("search-history-button").ghost().xsmall().icon(IconName::Clock).tooltip("Recent searches (↑ in an empty field)"))
            .content(move |_, _window, cx| {
                let muted = cx.theme().muted_foreground;
                let mut list = gpui_component::v_flex().w(px(320.0)).gap_0p5().p_1().text_sm();
                for (ix, item) in items.iter().enumerate() {
                    let (pick, remove) = (view.clone(), view.clone());
                    let (picked, removed) = (item.clone(), item.clone());
                    list = list.child(
                        h_flex()
                            .id(("search-history-item", ix))
                            .px_2()
                            .py_1()
                            .gap_2()
                            .items_center()
                            .rounded_md()
                            .cursor_pointer()
                            .hover(|s| s.bg(cx.theme().secondary))
                            .on_click(move |_, window, cx| {
                                pick.update(cx, |this, cx| this.pick_search_history(&picked, window, cx))
                            })
                            .child(div().flex_1().min_w(px(0.0)).truncate().font_family(cx.theme().mono_font_family.clone()).child(item.clone()))
                            .child(
                                Button::new(("search-history-remove", ix))
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::X)
                                    .text_color(muted)
                                    .tooltip("Forget this search")
                                    .on_click(move |_, _, cx| {
                                        cx.stop_propagation();
                                        remove.update(cx, |this, cx| this.remove_search_history(&removed, cx))
                                    }),
                            ),
                    );
                }
                list
            })
            .into_any_element()
    }
}
