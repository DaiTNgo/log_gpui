//! UI: status bar.

use super::AppView;
use crate::theme::ZED;
use gpui::{
    div, px, Context, InteractiveElement, IntoElement,
    ParentElement, StatefulInteractiveElement, Styled,
};

impl AppView {
    /// Status bar: line number, event counts, filter ratio, focus tag and tail toggle.
    pub(super) fn render_status_bar(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let selected_entry = self.selected_entry();
        let total_count = self.all_entries.len();
        let visible_count = self.visible_indices.len();
        let _filter_pct = if total_count > 0 {
            100.0 - (visible_count as f64 / total_count as f64) * 100.0
        } else {
            0.0
        };
        let _current_line_num = selected_entry.map(|e| e.index).unwrap_or(0);

        let total_count = self.all_entries.len();
        let visible_count = self.visible_indices.len();
        let filter_pct = if total_count > 0 {
            100.0 - (visible_count as f64 / total_count as f64) * 100.0
        } else {
            0.0
        };

        let current_line_num = selected_entry.map(|e| e.index).unwrap_or(0);

        div()
            .h(px(24.0))
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px_3()
            .bg(ZED.surface_background)
            .border_t_1()
            .border_color(ZED.border_variant)
            .text_xs()
            .font_family(".AppleSystemUIFontMonospaced")
            .text_color(ZED.text_muted)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2p5()
                    .child(div().child(format!("Ln {}", current_line_num)))
                    .child(div().text_color(ZED.border_variant).child("·"))
                    .child(div().child(format!("{},{:03} events", total_count / 1000, total_count % 1000)))
                    .child(div().text_color(ZED.border_variant).child("·"))
                    .child(if filter_pct > 0.0 {
                        div()
                            .text_color(ZED.text_accent)
                            .child(format!("Filtered {:.1}%", filter_pct))
                            .into_any_element()
                    } else {
                        div().child("All Visible").into_any_element()
                    })
                    .child(div().text_color(ZED.border_variant).child("·"))
                    .child(div().child("UTF-8"))
                    .child(if let Some(ref t) = self.focused_tag {
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2p5()
                            .child(div().text_color(ZED.border_variant).child("·"))
                            .child(div().text_color(ZED.text_accent).child(format!("Focus: [{}]", t)))
                            .into_any_element()
                    } else {
                        div().into_any_element()
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .id("btn_status_tail")
                            .cursor_pointer()
                            .px_1p5()
                            .py_0p5()
                            .rounded_xs()
                            .hover(|s| s.bg(ZED.ghost_hover))
                            .text_color(if self.tail_enabled {
                                ZED.success
                            } else {
                                ZED.text_muted
                            })
                            .child(if self.tail_enabled {
                                "Tail: ON ●"
                            } else {
                                "Tail: OFF"
                            })
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.tail_enabled = !this.tail_enabled;
                                cx.notify();
                            })),
                    ),
            )
            .into_any_element()
    }
}
