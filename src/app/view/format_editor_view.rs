//! UI: the Log Format editor: name, template (with its error underlined once
//! the user stops typing), file name globs, built-in templates to start from,
//! and a preview on real lines with each field in its own color.

use super::AppView;
use crate::log_format::{BUILT_IN, TOKENS};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    Context, ElementId, HighlightStyle, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, StyledText, UnderlineStyle, div, px,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::Input;
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_component::tag::Tag;
use gpui_component::{ActiveTheme as _, Disableable as _, Sizable as _, h_flex, v_flex};

/// The color of the `i`-th field of a template.
pub fn field_color(i: usize) -> gpui::Hsla {
    let hue = (i as f32 * 0.137 + 0.55) % 1.0;
    gpui::hsla(hue, 0.65, 0.55, 1.0)
}

impl AppView {
    pub(super) fn render_format_editor(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let Some(editor) = self.format_editor.as_ref() else {
            return div().into_any_element();
        };
        let theme = cx.theme();
        let (bg, border, muted, danger) = (theme.popover, theme.border, theme.muted_foreground, theme.danger);
        let mono = theme.mono_font_family.clone();
        let name_error = editor.name.visible_error().cloned();
        let template_error = editor.template.visible_error().cloned();
        let template_text = editor.template_input.read(cx).value().to_string();
        let title = if editor.id.is_some() { "Edit Log Format" } else { "New Log Format" };
        let can_save = editor.can_save();
        let view = cx.entity();

        let underlined = template_error.as_ref().map(|e| {
            let span = e.span.start.min(template_text.len())..e.span.end.min(template_text.len());
            let span = if span.is_empty() { template_text.len().saturating_sub(1)..template_text.len() } else { span };
            div().font_family(mono.clone()).text_xs().child(StyledText::new(template_text.clone()).with_highlights(vec![(
                span,
                HighlightStyle {
                    color: Some(danger),
                    underline: Some(UnderlineStyle { thickness: px(1.5), color: Some(danger), wavy: true }),
                    ..Default::default()
                },
            )]))
        });

        let preview = editor.preview.clone().map(|p| {
            let legend = h_flex().gap_1().flex_wrap().children(p.field_names.iter().enumerate().map(|(i, n)| {
                div().px_1().rounded_sm().text_xs().bg(field_color(i).opacity(0.3)).child(n.clone())
            }));
            let rows = p.lines.iter().map(|l| {
                let entry = l.entry;
                let highlights: Vec<_> = l
                    .fields
                    .iter()
                    .map(|(r, i)| (r.clone(), HighlightStyle { background_color: Some(field_color(*i).opacity(0.3)), ..Default::default() }))
                    .collect();
                h_flex()
                    .id(ElementId::from(format!("format-preview-{entry}")))
                    .gap_2()
                    .px_2()
                    .cursor_pointer()
                    .hover(|s| s.bg(muted.opacity(0.12)))
                    .when(!l.matched, |d| d.opacity(0.7))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .font_family(mono.clone())
                            .text_xs()
                            .child(StyledText::new(l.text.clone()).with_highlights(highlights)),
                    )
                    .when(!l.matched, |d| d.child(Tag::danger().xsmall().child("not matched")))
                    .on_click(cx.listener(move |this, _, _, cx| this.jump_to_preview_line(entry, cx)))
            });
            v_flex()
                .gap_1()
                .p_2()
                .rounded_md()
                .border_1()
                .border_color(border)
                .when(editor.stale, |d| d.opacity(0.5))
                .when(editor.stale, |d| d.child(div().text_xs().text_color(muted).child("Showing last valid pattern")))
                .child(legend)
                .child(v_flex().id("format-preview-lines").max_h(px(300.0)).overflow_y_scroll().children(rows))
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(div().flex_1().text_xs().text_color(muted).child(p.summary()))
                        .when(p.first_unmatched.is_some(), |d| {
                            d.child(
                                Button::new("format-first-unmatched")
                                    .ghost()
                                    .xsmall()
                                    .label("Go to first line not matched")
                                    .on_click(cx.listener(|this, _, _, cx| this.jump_to_first_unmatched(cx))),
                            )
                        }),
                )
        });

        let tokens: Vec<&str> = TOKENS.iter().map(|(t, _)| *t).collect();
        Self::modal_backdrop()
            .child(
                v_flex()
                    .id("format-editor")
                    .w(px(720.0))
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
                            this.close_format_editor(cx);
                        }
                    }))
                    .child(
                        h_flex()
                            .child(div().flex_1().text_base().font_weight(gpui::FontWeight::SEMIBOLD).child(title))
                            .child(
                                Button::new("format-start-from").small().label("Start from a template").dropdown_menu(
                                    move |menu, _, _| {
                                        let mut menu = menu;
                                        for (ix, (name, _, _)) in BUILT_IN.iter().enumerate() {
                                            let view = view.clone();
                                            menu = menu.item(PopupMenuItem::new(*name).on_click(move |_, window, app| {
                                                view.update(app, |this, cx| this.use_built_in_format(ix, window, cx));
                                            }));
                                        }
                                        menu
                                    },
                                ),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(div().text_sm().child("Name"))
                            .child(Input::new(&editor.name_input).when(name_error.is_some(), |i| i.border_color(danger)))
                            .when_some(name_error, |d, m| d.child(Self::field_error_row(m, danger))),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(div().text_sm().child("Template"))
                            .child(
                                Input::new(&editor.template_input)
                                    .when(template_error.is_some(), |i| i.border_color(danger)),
                            )
                            .children(underlined)
                            .when_some(template_error, |d, e| d.child(Self::field_error_row(e.message, danger)))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted)
                                    .child(format!("%{{TOKEN:field}} reads a field; (?<field>…) is a regex field. Tokens: {}", tokens.join(" "))),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(div().text_sm().child("Use for files named"))
                            .child(Input::new(&editor.globs_input)),
                    )
                    .child(v_flex().gap_1().child(div().text_sm().child("Preview")).children(preview))
                    .child(
                        h_flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("format-cancel")
                                    .label("Cancel")
                                    .on_click(cx.listener(|this, _, _, cx| this.close_format_editor(cx))),
                            )
                            .child(
                                Button::new("format-save")
                                    .primary()
                                    .label("Save")
                                    .disabled(!can_save)
                                    .on_click(cx.listener(|this, _, _, cx| this.save_format_editor(cx))),
                            ),
                    ),
            )
            .into_any_element()
    }
}
