//! UI: the Open in full dialog.

use super::AppView;
use crate::app::open_in_full::EstimateState;
use crate::live::buffer::size_text;
use crate::workspace::log_space::LogSpaceKind;
use gpui::prelude::FluentBuilder as _;
use gpui::{IntoElement, ParentElement, SharedString, Styled, div, px};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::checkbox::Checkbox;
use gpui_component::Disableable as _;
use gpui_component::input::Input;
use gpui_component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};

impl AppView {
    pub(super) fn render_open_in_full(&mut self, cx: &mut gpui::Context<Self>) -> gpui::AnyElement {
        let limit = self.live.full_read_limit;
        let Some(dialog) = self.remote.full_dialog.as_ref() else { return div().into_any_element() };
        let theme = cx.theme();
        let (bg, border, muted, danger) = (theme.popover, theme.border, theme.muted_foreground, theme.danger);

        let mut files = v_flex().gap_2();
        for (fi, file) in dialog.files.iter().enumerate() {
            let mut block = v_flex().gap_1().child(
                div().text_sm().font_weight(gpui::FontWeight::SEMIBOLD).child(file.name()),
            );
            if file.loading {
                block = block.child(div().text_xs().text_color(muted).child("Looking at the folder…"));
            } else if let Some(error) = &file.error {
                block = block.child(div().text_xs().text_color(danger).child(error.clone()));
            } else {
                block = block.child(
                    div()
                        .text_xs()
                        .text_color(muted)
                        .child(format!("The file itself: {}", size_text(file.size))),
                );
                if file.rotated.is_empty() {
                    block = block.child(div().text_xs().text_color(muted).child("No Rotated Files next to it"));
                }
                for (ri, rotated) in file.rotated.iter().enumerate() {
                    let ticked = file.ticked.get(ri).copied().unwrap_or(false);
                    let supported = rotated.compression.is_supported();
                    let mut label = format!("{} ({})", rotated.name, size_text(rotated.size));
                    if !supported {
                        label.push_str(" — compression not supported");
                    } else if dialog.outside_range(fi, ri) {
                        label.push_str(" — before the Starting Time");
                    }
                    block = block.child(
                        Checkbox::new(SharedString::from(format!("full-rotated-{fi}-{ri}")))
                            .label(label)
                            .checked(ticked)
                            .disabled(!supported)
                            .on_click(cx.listener(move |this, _, _, cx| this.toggle_full_rotated(fi, ri, cx))),
                    );
                }
            }
            files = files.child(block);
        }

        let start_error = dialog.start_error();
        let to_error = dialog.to_error();
        let over = dialog.over_limit(limit);
        let estimate_line = match &dialog.estimate {
            EstimateState::Working => div().text_xs().text_color(muted).child("Estimating…"),
            EstimateState::Failed(message) => div().text_xs().text_color(danger).child(message.clone()),
            EstimateState::Ready { total, .. } => {
                let text = if over {
                    format!(
                        "{}, above the Full Read Limit of {}",
                        total.text(),
                        crate::remote::estimate::short_count(limit as u64)
                    )
                } else {
                    total.text()
                };
                div().text_xs().text_color(if over { danger } else { muted }).child(text)
            }
        };
        let disk_error = dialog.disk_error.clone();
        let ready = matches!(dialog.estimate, EstimateState::Ready { .. });

        // Where the copy goes: a new Log Space, or one that is Imported.
        let imported: Vec<(String, String)> = self
            .active_project()
            .map(|p| {
                p.log_spaces
                    .iter()
                    .filter(|s| s.kind == LogSpaceKind::Imported)
                    .map(|s| (s.id.clone(), s.name.clone()))
                    .collect()
            })
            .unwrap_or_default();
        let existing = dialog.existing.clone();
        let mut destination = v_flex().gap_1().child(
            Checkbox::new("full-dest-new")
                .label("A new Log Space")
                .checked(existing.is_none())
                .on_click(cx.listener(|this, _, _, cx| this.choose_full_destination(None, cx))),
        );
        if existing.is_none() {
            if let Some(input) = dialog.name_input.clone() {
                destination = destination.child(Input::new(&input));
            }
        }
        for (id, name) in imported {
            let checked = existing.as_deref() == Some(id.as_str());
            let target = id.clone();
            destination = destination.child(
                Checkbox::new(SharedString::from(format!("full-dest-{id}")))
                    .label(format!("Add to {name}"))
                    .checked(checked)
                    .on_click(cx.listener(move |this, _, _, cx| this.choose_full_destination(Some(target.clone()), cx))),
            );
        }

        let start_input = dialog.start_input.clone();
        let to_input = dialog.to_input.clone();
        let title = if dialog.files.len() == 1 {
            format!("Open {} in full", dialog.files[0].name())
        } else {
            format!("Open {} files in full", dialog.files.len())
        };
        let can_open = ready && !over && start_error.is_none() && to_error.is_none() && disk_error.is_none();

        Self::modal_backdrop()
            .child(
                v_flex()
                    .w(px(560.0))
                    .max_h(px(640.0))
                    .overflow_hidden()
                    .gap_3()
                    .p_4()
                    .bg(bg)
                    .border_1()
                    .border_color(border)
                    .rounded_lg()
                    .shadow_2xl()
                    .child(div().text_base().font_weight(gpui::FontWeight::SEMIBOLD).child(title))
                    .child(div().text_xs().text_color(muted).child(
                        "Reads the whole file, and the Rotated Files you tick, into a copy kept in this app. The Server's files are not changed.",
                    ))
                    .child(files)
                    .child(
                        v_flex()
                            .gap_1()
                            .child(div().text_sm().child("Time range (on the clock of the file)"))
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(div().flex_1().when_some(start_input, |d, i| d.child(Input::new(&i))))
                                    .child(div().flex_1().when_some(to_input, |d, i| d.child(Input::new(&i)))),
                            )
                            .when_some(start_error, |d, e| d.child(div().text_xs().text_color(danger).child(format!("From: {e}"))))
                            .when_some(to_error, |d, e| d.child(div().text_xs().text_color(danger).child(format!("To: {e}")))),
                    )
                    .child(destination)
                    .child(estimate_line)
                    .when_some(disk_error, |d, e| d.child(div().text_xs().text_color(danger).child(e)))
                    .child(
                        h_flex()
                            .justify_end()
                            .gap_2()
                            .when(over, |row| {
                                row.child(
                                    Button::new("full-narrow")
                                        .label("Narrow time…")
                                        .on_click(cx.listener(|this, _, _, cx| this.full_narrow_time(cx))),
                                )
                                .child(
                                    Button::new("full-newest")
                                        .primary()
                                        .label(format!(
                                            "Read newest {} lines",
                                            crate::remote::estimate::short_count(limit as u64)
                                        ))
                                        .on_click(cx.listener(|this, _, _, cx| this.full_read_newest(cx))),
                                )
                            })
                            .child(
                                Button::new("full-cancel")
                                    .label("Cancel")
                                    .on_click(cx.listener(|this, _, _, cx| this.close_open_in_full(cx))),
                            )
                            .when(!over, |row| {
                                row.child(
                                    Button::new("full-open")
                                        .primary()
                                        .label("Open")
                                        .disabled(!can_open)
                                        .on_click(cx.listener(|this, _, _, cx| this.confirm_open_in_full(cx))),
                                )
                            }),
                    ),
            )
            .into_any_element()
    }
}
