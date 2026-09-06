use crate::model::LogEntry;
use crate::theme::CARBON;
use gpui::{
    div, px, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled,
};
use gpui_component::{Icon, IconName};

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

            let level_color = match entry.level {
                crate::model::LogLevel::Fatal | crate::model::LogLevel::Error => CARBON.error,
                crate::model::LogLevel::Warn => CARBON.warning,
                crate::model::LogLevel::Info => CARBON.info,
                crate::model::LogLevel::Debug => CARBON.success,
                _ => CARBON.text_muted,
            };

            let row = div()
                .flex()
                .flex_row()
                .items_center()
                .gap_3()
                .px_2()
                .py_1()
                .rounded_sm()
                .bg(if is_selected {
                    CARBON.layer_selected
                } else {
                    CARBON.bg
                })
                .border_1()
                .border_color(if is_selected {
                    CARBON.border_interactive
                } else {
                    CARBON.transparent
                })
                .font_family(".AppleSystemUIFontMonospaced")
                .text_xs()
                .child(
                    div()
                        .w(px(50.0))
                        .text_color(if is_selected {
                            CARBON.blue_primary
                        } else {
                            CARBON.text_muted
                        })
                        .font_weight(if is_selected {
                            gpui::FontWeight::BOLD
                        } else {
                            gpui::FontWeight::NORMAL
                        })
                        .child(format!("#{}", entry.index)),
                )
                .child(
                    div()
                        .w(px(90.0))
                        .text_color(CARBON.text_muted)
                        .child(entry.time_str.clone()),
                )
                .child(
                    div()
                        .w(px(56.0))
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(level_color)
                        .child(entry.level.as_str()),
                )
                .child(
                    div()
                        .w(px(80.0))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(CARBON.purple_accent)
                        .child(entry.tag.clone()),
                )
                .child(
                    div()
                        .flex_1()
                        .text_color(if is_selected {
                            CARBON.text_primary
                        } else {
                            CARBON.text_secondary
                        })
                        .truncate()
                        .child(entry.payload.clone()),
                )
                .child(if is_selected {
                    div()
                        .px_1p5()
                        .py_0p5()
                        .rounded_xs()
                        .bg(CARBON.blue_primary)
                        .text_color(CARBON.text_on_color)
                        .text_xs()
                        .font_weight(gpui::FontWeight::BOLD)
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
            .bg(gpui::rgba(0x000000bb))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(px(800.0))
                    .max_h(px(550.0))
                    .bg(CARBON.layer_01)
                    .border_1()
                    .border_color(CARBON.border_strong)
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
                            .border_color(CARBON.border_subtle)
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
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(CARBON.text_primary)
                                            .child("SURROUNDING CONTEXT"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(CARBON.text_muted)
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
                                    .rounded_sm()
                                    .text_xs()
                                    .text_color(CARBON.text_muted)
                                    .hover(|s| s.text_color(CARBON.text_primary))
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
                            .border_color(CARBON.border_subtle)
                            .text_xs()
                            .text_color(CARBON.text_muted)
                            .child("Showing ±5 lines from raw log stream to inspect what happened immediately prior to and after this event."),
                    ),
            )
            .into_any_element()
    }
}
