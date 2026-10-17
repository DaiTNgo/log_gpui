//! UI: view panel content (density, columns, module focus).

use super::AppView;
use gpui_component::Icon;
use gpui_kit::assets::IconName;
use crate::column::PinnableColumn;
use crate::density::Density;
use gpui::{
    div, Context, ElementId, InteractiveElement, IntoElement,
    ParentElement, StatefulInteractiveElement, Styled,
};

impl AppView {
    /// View panel: density, pinned columns, and module focus.
    pub fn render_view_panel_content(
        &mut self,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let density_btn = |label: &'static str, target: Density| {
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
                .border_1()
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
                .child(
                    div()
                        .text_xs()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .child(col_label),
                )
                .child(Icon::new(if pinned { IconName::Pin } else { IconName::PinOff }).size_3())
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
                .child(
                    div()
                        .text_xs()
                        .font_weight(gpui::FontWeight::MEDIUM)
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
                    .child("MODULE FOCUS"),
            )
            .child(modules)
            .into_any_element()
    }
}
