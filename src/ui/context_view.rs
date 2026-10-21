use crate::model::LogEntry;

use gpui_component::ActiveTheme as _;
use gpui::{
    div, px, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled,
};
use gpui_component::{Icon};
use gpui_kit::assets::IconName;

pub struct ContextModal;

impl ContextModal {
    pub fn render(
        is_open: bool,
        selected_entry_ix: Option<usize>,
        all_entries: &[LogEntry],
        cx: &mut gpui::Context<crate::app::AppView>,
    ) -> gpui::AnyElement {
        if !is_open {
            return div().into_any_element();
        }

        let Some(target_ix) = selected_entry_ix else {
            return div().into_any_element();
        };
        let mono_font = cx.theme().mono_font_family.clone();

        if target_ix >= all_entries.len() {
            return div().into_any_element();
        }

        let target_entry = &all_entries[target_ix];

        // Retrieve surrounding 5 lines before and 5 lines after in original stream
        let start_ix = target_ix.saturating_sub(5);
        let end_ix = (target_ix + 6).min(all_entries.len());

        let mut lines_list = div().flex().flex_col().gap_0p5().w_full();

        for ix in start_ix..end_ix {
            let entry = &all_entries[ix];
            let is_selected = ix == target_ix;


            let row = div()
                .flex()
                .flex_row()
                .items_center()
                .gap_3()
                .px_2()
                .py_1()
                .rounded_md()
                .border_1()
                .font_family(mono_font.clone())
                .text_xs()
                .child(
                    div()
                        .w(px(50.0))
                        .font_weight(if is_selected {
                            gpui::FontWeight::SEMIBOLD
                        } else {
                            gpui::FontWeight::NORMAL
                        })
                        .child(format!("#{}", entry.index)),
                )
                .child(
                    div()
                        .w(px(90.0))
                        .child(entry.time_str.clone()),
                )
                .child(
                    div()
                        .w(px(56.0))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(entry.level.as_str()),
                )
                .child(
                    div()
                        .w(px(80.0))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(entry.tag.clone()),
                )
                .child(
                    div()
                        .flex_1()
                        .truncate()
                        .child(entry.payload.clone()),
                )
                .child(if is_selected {
                    div()
                        .px_1p5()
                        .py_0p5()
                        .rounded_xs()
                        .border_1()
                        .text_xs()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child("SELECTED")
                        .into_any_element()
                } else {
                    div().into_any_element()
                });

            lines_list = lines_list.child(row);
        }

        // Overlay backdrop + Modal
        div()
            .absolute()
            .inset_0()
            .bg(gpui::rgba(0x00000088))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(px(800.0))
                    .max_h(px(550.0))
                    .bg(cx.theme().popover)
                    .border_1()
                    .border_color(cx.theme().border)
                    .text_color(cx.theme().popover_foreground)
                    .rounded_lg()
                    .shadow_2xl()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .border_b_1()
                            .pb_2()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .child("SURROUNDING CONTEXT"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .child(format!(
                                                "Unfiltered chronological log stream around Line #{}",
                                                target_entry.index
                                            )),
                                    ),
                            )
                            .child(
                                div()
                                    .id("close_context_modal")
                                    .cursor_pointer()
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .text_xs()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap_1()
                                            .child(Icon::new(IconName::Close).size(px(12.0)))
                                            .child("Close (Esc)"),
                                    )
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        this.is_context_open = false;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(lines_list)
                    .child(
                        div()
                            .pt_2()
                            .border_t_1()
                            .text_xs()
                            .child("Showing ±5 lines from raw log stream to inspect what happened immediately prior to and after this event."),
                    ),
            )
            .into_any_element()
    }
}
