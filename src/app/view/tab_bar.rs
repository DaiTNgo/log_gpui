//! UI: working-set tab bar.

use super::AppView;
use crate::theme::ZED;
use crate::workspace::working_set::TabTarget;
use gpui::{
    div, px, Context, ElementId, InteractiveElement, IntoElement, MouseButton,
    ParentElement, StatefulInteractiveElement, Styled,
};

impl AppView {
    pub(super) fn render_tab_bar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let tabs_data: Vec<(usize, String, bool, bool, bool)> = {
            match self.active_project() {
                Some(proj) => {
                    let active_idx = proj.working_set.active_tab_index();
                    proj.working_set
                        .tabs()
                        .iter()
                        .enumerate()
                        .map(|(index, tab)| {
                            let label = match &tab.target {
                                TabTarget::LogFile { file_id } => proj
                                    .file(file_id)
                                    .map(|f| f.display_name.clone())
                                    .unwrap_or_else(|| file_id.clone()),
                                TabTarget::InvestigationBoard { case_id } => {
                                    format!("⚑ {}", case_id)
                                }
                                TabTarget::ChronicleStream { file_ids } => {
                                    format!("⇄ {} files", file_ids.len())
                                }
                            };
                            let (error, warn) = match &tab.target {
                                TabTarget::LogFile { file_id } => self.scan_hazards(file_id),
                                _ => (false, false),
                            };
                            (index, label, Some(index) == active_idx, error, warn)
                        })
                        .collect()
                }
                None => Vec::new(),
            }
        };

        let mut bar = div()
            .h(px(30.0))
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_3()
            .bg(ZED.surface_background)
            .border_b_1()
            .border_color(ZED.border_variant)
            .overflow_x_hidden()
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1p5(),
            );

        let mut list_tabs = div().flex().flex_row().items_center().gap_1p5();
        for (index, label, active, has_error, has_warn) in &tabs_data {
            let index = *index;
            let label = label.clone();
            let active = *active;
            let has_error = *has_error;
            let has_warn = *has_warn;
            let pill = div()
                .id(ElementId::from(format!("tab_{}", index)))
                .cursor_pointer()
                .flex()
                .flex_row()
                .items_center()
                .gap_1p5()
                .px_2()
                .py_0p5()
                .rounded_md()
                .bg(if active {
                    ZED.element_selected
                } else {
                    ZED.transparent
                })
                .hover(|s| s.bg(ZED.ghost_hover))
                .text_xs()
                .text_color(if active {
                    ZED.text
                } else {
                    ZED.text_muted
                })
                .font_weight(if active {
                    gpui::FontWeight::SEMIBOLD
                } else {
                    gpui::FontWeight::NORMAL
                })
                .child(
                    div()
                        .w(px(6.0))
                        .h(px(6.0))
                        .rounded_full()
                        .bg(if has_error {
                            ZED.error
                        } else if has_warn {
                            ZED.warning
                        } else {
                            ZED.transparent
                        }),
                )
                .child(label.clone())
                .child(
                    div()
                        .id(ElementId::from(format!("tab_close_{}", index)))
                        .cursor_pointer()
                        .px_0p5()
                        .text_color(ZED.text_muted)
                        .hover(|s| s.text_color(ZED.error))
                        .child("×")
                        .on_click(cx.listener(move |this, _, _window, cx| {
                            this.close_tab_at(index, cx);
                        })),
                )
                .on_mouse_down(
                    MouseButton::Middle,
                    cx.listener(move |this, _, _window, cx| {
                        this.close_tab_at(index, cx);
                    }),
                )
                .on_click(cx.listener(move |this, _, _window, cx| {
                    this.switch_to_tab(index, cx);
                }));

            list_tabs = list_tabs.child(pill);
        }

        if tabs_data.is_empty() {
            bar = bar
                .child(
                    div()
                        .text_xs()
                        .text_color(ZED.text_muted)
                        .child("No tabs open — open a log file from the Files drawer"),
                )
                .child(div().flex_1());
        } else {
            bar = bar.child(list_tabs);
        }

        bar
    }
}
