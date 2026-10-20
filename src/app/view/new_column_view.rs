//! UI: the New Column dialog: Name, the ways to read the value (as words),
//! the regex on "Edit regex", and a Preview on real lines.

use super::AppView;
use crate::extract::PreviewLine;
use gpui::prelude::FluentBuilder as _;
use gpui::{
    Context, ElementId, HighlightStyle, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, StyledText, div, px,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::Input;
use gpui_component::tag::Tag;
use gpui_component::{ActiveTheme as _, Disableable as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::assets::IconName;

impl AppView {
    pub(super) fn render_new_column(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let Some(dialog) = self.new_column.as_ref() else {
            return div().into_any_element();
        };
        let theme = cx.theme();
        let (bg, border, muted, danger, primary) =
            (theme.popover, theme.border, theme.muted_foreground, theme.danger, theme.primary);
        let mono = theme.mono_font_family.clone();
        let name_error = dialog.name.visible_error().cloned();
        let regex_error = dialog.pattern.visible_error().map(|e| e.message());
        let can_save = dialog.can_save();
        let title = if dialog.editing.is_some() { "Edit rule" } else { "New column" };
        let save_label = if dialog.editing.is_some() { "Save" } else { "Add column" };

        let mut ways = v_flex().gap_1();
        for (ix, c) in dialog.candidates.iter().enumerate() {
            let chosen = dialog.chosen == Some(ix);
            ways = ways.child(
                h_flex()
                    .id(ElementId::from(format!("new-column-way-{ix}")))
                    .gap_2()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(if chosen { primary } else { border })
                    .when(chosen, |d| d.bg(primary.opacity(0.08)))
                    .cursor_pointer()
                    .child(
                        Icon::new(if chosen { IconName::CircleCheck } else { IconName::Circle })
                            .size_3()
                            .text_color(if chosen { primary } else { muted }),
                    )
                    .child(div().flex_1().text_sm().child(c.description.clone()))
                    .child(div().text_xs().text_color(muted).child(format!("{} sampled lines", c.coverage)))
                    .on_click(cx.listener(move |this, _, window, cx| this.choose_new_column_candidate(ix, window, cx))),
            );
        }
        if dialog.chosen.is_none() && !dialog.candidates.is_empty() {
            ways = ways.child(div().text_xs().text_color(muted).child("Using your own regex"));
        }

        let preview = dialog.preview.clone().map(|p| {
            let line = |pl: &PreviewLine, matched: bool| {
                let row = pl.row;
                let text = StyledText::new(pl.text.clone()).with_highlights(if matched {
                    vec![(pl.highlight.clone(), HighlightStyle { background_color: Some(primary.opacity(0.35)), ..Default::default() })]
                } else {
                    Vec::new()
                });
                h_flex()
                    .id(ElementId::from(format!("new-column-preview-{row}-{matched}")))
                    .gap_2()
                    .px_2()
                    .py(px(2.0))
                    .rounded_sm()
                    .cursor_pointer()
                    .hover(|s| s.bg(muted.opacity(0.12)))
                    .child(div().flex_1().min_w(px(0.0)).overflow_hidden().whitespace_nowrap().font_family(mono.clone()).text_xs().child(text))
                    .when(!matched, |d| d.child(Tag::danger().xsmall().child("not matched")))
                    .on_click(cx.listener(move |this, _, _, cx| this.jump_to_preview_line(row, cx)))
            };
            v_flex()
                .gap_1()
                .p_2()
                .rounded_md()
                .border_1()
                .border_color(border)
                .when(dialog.stale, |d| d.opacity(0.5))
                .when(dialog.stale, |d| d.child(div().text_xs().text_color(muted).child("Showing last valid pattern")))
                .children(p.matched.iter().map(|l| line(l, true)))
                .children(p.unmatched.iter().map(|l| line(l, false)))
                .child(div().pt_1().text_xs().text_color(muted).child(p.summary()))
                .when(!p.top_values.is_empty(), |d| {
                    d.child(h_flex().gap_1().flex_wrap().children(
                        p.top_values.iter().map(|(v, n)| Tag::secondary().xsmall().child(format!("{v}  {n}"))),
                    ))
                })
        });

        Self::modal_backdrop()
            .child(
                v_flex()
                    .id("new-column-dialog")
                    .w(px(600.0))
                    .gap_3()
                    .p_4()
                    .bg(bg)
                    .border_1()
                    .border_color(border)
                    .rounded_lg()
                    .shadow_2xl()
                    .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _window, cx| {
                        if event.keystroke.key == "escape" {
                            cx.stop_propagation();
                            this.close_new_column(cx);
                        }
                    }))
                    .child(div().text_base().font_weight(gpui::FontWeight::SEMIBOLD).child(title))
                    .child(
                        v_flex()
                            .gap_1()
                            .child(div().text_sm().child("Name"))
                            .child(Input::new(&dialog.name_input).when(name_error.is_some(), |i| i.border_color(danger)))
                            .when_some(name_error, |d, message| d.child(Self::field_error_row(message, danger)))
                            .when_some(dialog.name_hint.clone(), |d, hint| {
                                d.child(div().text_xs().text_color(muted).child(hint))
                            }),
                    )
                    .when(!dialog.candidates.is_empty(), |d| {
                        d.child(v_flex().gap_1().child(div().text_sm().child("Read the value")).child(ways))
                    })
                    .child(
                        h_flex().child(
                            Button::new("new-column-edit-regex")
                                .ghost()
                                .xsmall()
                                .label(if dialog.regex_open { "Hide regex" } else { "Edit regex" })
                                .on_click(cx.listener(|this, _, _, cx| this.toggle_new_column_regex(cx))),
                        ),
                    )
                    .when(dialog.regex_open, |d| {
                        d.child(
                            v_flex()
                                .gap_1()
                                .child(
                                    Input::new(&dialog.regex_input)
                                        .when(regex_error.is_some(), |i| i.border_color(danger)),
                                )
                                .when_some(regex_error, |d, message| d.child(Self::field_error_row(message, danger))),
                        )
                    })
                    .child(v_flex().gap_1().child(div().text_sm().child("Preview")).children(preview))
                    .child(
                        h_flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("new-column-cancel")
                                    .label("Cancel")
                                    .on_click(cx.listener(|this, _, _, cx| this.close_new_column(cx))),
                            )
                            .child(
                                Button::new("new-column-add")
                                    .primary()
                                    .label(save_label)
                                    .disabled(!can_save)
                                    .on_click(cx.listener(|this, _, _, cx| this.confirm_new_column(cx))),
                            ),
                    ),
            )
            .into_any_element()
    }
}
