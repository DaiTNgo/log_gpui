//! UI: editor toolbar.

use super::AppView;
use crate::theme::ZED;
use crate::ui::floating_panel::PanelKind;
use gpui::{
    div, px, Context, ElementId, InteractiveElement, IntoElement, MouseButton,
    ParentElement, SharedString, StatefulInteractiveElement, Styled, prelude::FluentBuilder as _,
};
use gpui_component::{input::Editor, Icon, IconName};

impl AppView {
    /// Editor toolbar: search field, active filter chips and panel toggles.
    pub(super) fn render_editor_toolbar(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let mut filter_chips = div().flex().flex_row().items_center().gap_1p5();

        for cond in &self.filter_engine.conditions {
            if !cond.enabled || cond.value.trim().is_empty() {
                continue;
            }
            let cond_id = cond.id;
            let label = cond.to_chip_label();

            let chip = div()
                .id(ElementId::from(format!("chip_{}", cond_id)))
                .cursor_pointer()
                .flex()
                .flex_row()
                .items_center()
                .gap_1p5()
                .px_2()
                .py_0p5()
                .rounded_md()
                .bg(ZED.element_background)
                .border_1()
                .border_color(ZED.border_variant)
                .text_xs()
                .text_color(ZED.text_accent)
                .child(label)
                .child(
                    div()
                        .text_xs()
                        .text_color(ZED.text_muted)
                        .hover(|s| s.text_color(ZED.error))
                        .child("×"),
                )
                .on_click(cx.listener(move |this, _, _window, cx| {
                    this.remove_filter_condition(cond_id, cx);
                }));

            filter_chips = filter_chips.child(chip);
        }

        div()
            .h(px(34.0))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px_3()
            .bg(ZED.surface_background)
            .border_b_1()
            .border_color(ZED.border_variant)
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    // Full-width search field: query editor (query-LSP completions)
                    .child(
                        div()
                            .id("toolbar_search_field")
                            .flex_1()
                            .min_w(px(0.0))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .px_2p5()
                            .py_1()
                            .rounded_md()
                            .bg(ZED.element_background)
                            .border_1()
                            .border_color(if self.search_state.is_open {
                                ZED.border_focused
                            } else {
                                ZED.border_variant
                            })
                            .hover(|s| s.border_color(ZED.border_focused))
                            .child(
                                div()
                                    .text_color(ZED.icon)
                                    .child(Icon::new(IconName::Search).size(px(13.0))),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .child(
                                        Editor::new(&self.search_input)
                                            .appearance(false)
                                            .bordered(false)
                                            .h(px(24.0)),
                                    ),
                            )
                            .when(
                                self.search_state.is_open && !self.search_state.query.is_empty(),
                                |this| {
                                    this.child(
                                        div()
                                            .id("toolbar_search_clear")
                                            .cursor_pointer()
                                            .px_1()
                                            .rounded_xs()
                                            .hover(|s| s.bg(ZED.ghost_hover))
                                            .text_color(ZED.text_muted)
                                            .child("×")
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.search_input.update(cx, |state, cx| {
                                                    state.replace_all(
                                                        SharedString::from(""),
                                                        window,
                                                        cx,
                                                    );
                                                });
                                                this.run_search(cx);
                                                this.search_input.update(cx, |state, cx| {
                                                    state.focus(window, cx);
                                                });
                                            })),
                                    )
                                },
                            )
                            .when(!self.search_state.is_open, |this| {
                                this.child(
                                    div()
                                        .px_1()
                                        .py_0p5()
                                        .rounded_xs()
                                        .bg(ZED.surface_background)
                                        .text_color(ZED.text_muted)
                                        .child("⌘F"),
                                )
                            })
                            .when(self.search_state.parse_error.is_some(), |this| {
                                let msg = self
                                    .search_state
                                    .parse_error
                                    .as_ref()
                                    .map(|e| e.message())
                                    .unwrap_or_default();
                                let shown = if msg.chars().count() > 48 {
                                    format!("! {}", msg.chars().take(48).collect::<String>())
                                } else {
                                    format!("! {}", msg)
                                };
                                this.child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap_1p5()
                                        .px_2()
                                        .py_0p5()
                                        .rounded_xs()
                                        .bg(ZED.error_bg)
                                        .text_xs()
                                        .font_family(".AppleSystemUIFontMonospaced")
                                        .text_color(ZED.error)
                                        .child(shown),
                                )
                            })
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    this.activate_search(window, cx);
                                }),
                            ),
                    )
                    // Active Filter Chips
                    .child(filter_chips)
                    // Tool panels: View / Filter / Marker
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1()
                            .child(
                                div()
                                    .id("toolbar_btn_view")
                                    .cursor_pointer()
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .bg(if self.panel(PanelKind::View).open {
                                        ZED.ghost_selected
                                    } else {
                                        ZED.transparent
                                    })
                                    .hover(|s| s.bg(ZED.ghost_hover))
                                    .text_xs()
                                    .text_color(if self.panel(PanelKind::View).open {
                                        ZED.text_accent
                                    } else {
                                        ZED.text_muted
                                    })
                                    .child("View ⌘⇧V")
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        this.toggle_panel(PanelKind::View, cx);
                                    })),
                            )
                            .child(
                                div()
                                    .id("toolbar_btn_filter")
                                    .cursor_pointer()
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .bg(if self.panel(PanelKind::Filter).open {
                                        ZED.ghost_selected
                                    } else {
                                        ZED.transparent
                                    })
                                    .hover(|s| s.bg(ZED.ghost_hover))
                                    .text_xs()
                                    .text_color(if self.panel(PanelKind::Filter).open {
                                        ZED.text_accent
                                    } else {
                                        ZED.text_muted
                                    })
                                    .child("Filter ⌘⇧F")
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        this.toggle_filter_builder(cx);
                                    })),
                            )
                            .child(
                                div()
                                    .id("toolbar_btn_marker")
                                    .cursor_pointer()
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .bg(if self.panel(PanelKind::Marker).open {
                                        ZED.ghost_selected
                                    } else {
                                        ZED.transparent
                                    })
                                    .hover(|s| s.bg(ZED.ghost_hover))
                                    .text_xs()
                                    .text_color(if self.panel(PanelKind::Marker).open {
                                        ZED.text_accent
                                    } else {
                                        ZED.text_muted
                                    })
                                    .child("Marker ⌘⇧M")
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        this.toggle_panel(PanelKind::Marker, cx);
                                    })),
                            ),
                    )
                    // Inspector toggle
                    .child(
                        div()
                            .id("btn_toggle_inspector")
                            .cursor_pointer()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(if self.is_inspector_open {
                                ZED.ghost_selected
                            } else {
                                ZED.transparent
                            })
                            .hover(|s| s.bg(ZED.ghost_hover))
                            .text_xs()
                            .text_color(if self.is_inspector_open {
                                ZED.text_accent
                            } else {
                                ZED.text_muted
                            })
                            .child("Inspector ⌘I")
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.is_inspector_open = !this.is_inspector_open;
                                cx.notify();
                            })),
                    ),
            )
            .into_any_element()
    }
}
