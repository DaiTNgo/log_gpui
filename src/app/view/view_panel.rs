//! UI: view panel content (density, columns, module focus).

use super::AppView;
use crate::column::PinnableColumn;
use crate::theme::{Density, ZED};
use gpui::{
    div, px, Context, ElementId, InteractiveElement, IntoElement,
    ParentElement, StatefulInteractiveElement, Styled,
};

impl AppView {
    /// View panel: density, pinned columns, and module focus.
    pub fn render_view_panel_content(
        &mut self,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let density_btn = |label: &'static str, target: Density| {
            let active = self.density == target;
            div()
                .id(ElementId::from(format!("view_density_{}", label.to_lowercase())))
                .cursor_pointer()
                .flex_1()
                .px_2()
                .py_1()
                .rounded_md()
                .text_center()
                .text_xs()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .bg(if active {
                    ZED.element_selected
                } else {
                    ZED.element_background
                })
                .border_1()
                .border_color(if active {
                    ZED.border_focused
                } else {
                    ZED.border_variant
                })
                .text_color(if active {
                    ZED.text_accent
                } else {
                    ZED.text_muted
                })
                .hover(|s| s.bg(ZED.ghost_hover))
                .child(label)
                .on_click(cx.listener(move |this, _, _window, cx| {
                    this.column_layout.update_density(target);
                    this.density = target;
                    cx.notify();
                }))
        };

        let mut columns = div().flex().flex_col().gap_1().w_full();
        for col in PinnableColumn::ALL {
            let col_label = col.label();
            let pinned = self.column_layout.is_pinned(col);
            let row = div()
                .id(ElementId::from(format!("view_col_{}", col_label)))
                .cursor_pointer()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .px_2()
                .py_1()
                .rounded_md()
                .hover(|s| s.bg(ZED.ghost_hover))
                .child(
                    div()
                        .text_xs()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(ZED.text)
                        .child(col_label),
                )
                .child(
                    div()
                        .text_color(if pinned {
                            ZED.icon_accent
                        } else {
                            ZED.icon_muted
                        })
                        .child(
                            gpui::svg()
                                .size(px(13.0))
                                .text_color(if pinned {
                                    ZED.icon_accent
                                } else {
                                    ZED.icon_muted
                                })
                                .data(if pinned {
                                    crate::column::PUSH_PIN_SIMPLE_SVG
                                } else {
                                    crate::column::PUSH_PIN_SIMPLE_SLASH_SVG
                                }),
                        ),
                )
                .on_click(cx.listener(move |this, _, _window, cx| {
                    this.column_layout.toggle_pin(col);
                    cx.notify();
                }));
            columns = columns.child(row);
        }

        let mut modules = div().flex().flex_col().gap_1().w_full();
        let mod_list = ["API", "DB", "AUTH", "PAYMENT", "CACHE", "WORKER", "GATEWAY"];
        for mod_name in mod_list {
            let m_str = mod_name.to_string();
            let is_focused = self.focused_tag.as_deref() == Some(mod_name);
            let item = div()
                .id(ElementId::from(format!("view_mod_{}", mod_name)))
                .cursor_pointer()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .px_2()
                .py_1()
                .rounded_md()
                .bg(if is_focused {
                    ZED.element_selected
                } else {
                    ZED.transparent
                })
                .hover(|s| s.bg(ZED.ghost_hover))
                .child(
                    div()
                        .text_xs()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(if is_focused {
                            ZED.text_accent
                        } else {
                            ZED.text
                        })
                        .child(mod_name),
                )
                .on_click(cx.listener(move |this, _, _window, cx| {
                    if this.focused_tag.as_deref() == Some(&m_str) {
                        this.focused_tag = None;
                    } else {
                        this.focused_tag = Some(m_str.clone());
                    }
                    cx.notify();
                }));
            modules = modules.child(item);
        }

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_xs()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(ZED.text_muted)
                    .child("DENSITY"),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .child(density_btn("Comfortable", Density::Comfortable))
                    .child(density_btn("Compact", Density::Compact))
                    .child(density_btn("Ultra", Density::UltraCompact)),
            )
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
                            .text_color(ZED.text_muted)
                            .child("COLUMNS"),
                    )
                    .child(
                        div()
                            .id("view_col_unpin_all")
                            .cursor_pointer()
                            .px_1p5()
                            .py_0p5()
                            .rounded_xs()
                            .text_xs()
                            .text_color(ZED.text_muted)
                            .hover(|s| s.bg(ZED.ghost_hover).text_color(ZED.text))
                            .child("Unpin All")
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.column_layout.unpin_all();
                                cx.notify();
                            })),
                    ),
            )
            .child(columns)
            .child(
                div()
                    .text_xs()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(ZED.text_muted)
                    .child("MODULE FOCUS"),
            )
            .child(modules)
            .into_any_element()
    }
}
