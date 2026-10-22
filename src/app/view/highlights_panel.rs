//! UI: Highlights section of the context sidebar. Each Highlight Rule is a
//! row with its color, column, values, how many rows it matches, a switch, a
//! delete button and a handle to drag it up or down the priority list.

use super::AppView;
use crate::highlight::{HighlightRule, MESSAGE_COLUMN};
use crate::palette::{HighlightColor, Tone, palette_cells};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    AppContext as _, Context, ElementId, InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::checkbox::Checkbox;
use gpui_component::input::Input;
use gpui_component::switch::Switch;
use gpui_component::tag::Tag;
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::assets::IconName;

/// The payload of dragging a rule: its place in the list.
#[derive(Clone)]
pub struct DragRule {
    pub ix: usize,
    pub label: String,
}

impl Render for DragRule {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().popover)
            .text_xs()
            .child(self.label.clone())
    }
}

/// A square of palette color. `size` is in pixels.
pub fn swatch(id: impl Into<ElementId>, color: &HighlightColor, tone: Tone, size: f32) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .size(px(size))
        .flex_shrink_0()
        .rounded(px(3.0))
        .border_1()
        .border_color(gpui::black().opacity(0.25))
        .bg(gpui::rgb(color.rgb(tone)))
        .cursor_pointer()
}

impl AppView {
    pub(crate) fn tone(&self, cx: &gpui::App) -> Tone {
        if cx.theme().is_dark() { Tone::Dark } else { Tone::Light }
    }

    fn column_label(&self, key: &str) -> String {
        if key == MESSAGE_COLUMN {
            return "Message".to_string();
        }
        self.filter_index
            .column(key)
            .map_or_else(|| key.to_string(), |c| c.label.clone())
    }

    fn render_rule_row(&self, ix: usize, rule: &HighlightRule, cx: &mut Context<Self>) -> gpui::AnyElement {
        let tone = self.tone(cx);
        let id = rule.id;
        let matches = self.rule_counts.get(&rule.id).copied().unwrap_or(0);
        let column = self.column_label(&rule.column);
        let drag = DragRule { ix, label: format!("{column}: {}", rule.summary()) };
        let view = cx.entity();
        let enabled = rule.enabled;
        h_flex()
            .id(ElementId::from(format!("hl-rule-{id}")))
            .w_full()
            .gap_2()
            .px_1()
            .py_1()
            .items_center()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().border)
            .opacity(if enabled { 1.0 } else { 0.55 })
            .on_drag(drag, |drag, _, _, cx| cx.new(|_| drag.clone()))
            .drag_over::<DragRule>(|style, _, _, cx| style.border_color(cx.theme().primary))
            .on_drop(cx.listener(move |this, dragged: &DragRule, _window, cx| {
                this.move_highlight_rule(dragged.ix, ix, cx);
            }))
            .child(
                div()
                    .cursor_grab()
                    .text_color(cx.theme().muted_foreground)
                    .child(Icon::new(IconName::GripVertical).size(px(14.0))),
            )
            .child(
                swatch(ElementId::from(format!("hl-color-{id}")), &rule.color, tone, 18.0).on_click(
                    cx.listener(move |this, event: &gpui::ClickEvent, _window, cx| {
                        this.open_rule_palette(id, event.position(), cx);
                    }),
                ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w(px(0.0))
                    .child(div().text_xs().font_weight(gpui::FontWeight::SEMIBOLD).child(column))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .truncate()
                            .child(rule.summary()),
                    ),
            )
            .child(Tag::secondary().child(matches.to_string()))
            .child(
                Switch::new(ElementId::from(format!("hl-switch-{id}")))
                    .checked(enabled)
                    .on_click(move |_, _window, app| {
                        view.update(app, |this, cx| this.toggle_highlight_rule(id, cx));
                    }),
            )
            .child(
                Button::new(ElementId::from(format!("hl-delete-{id}")))
                    .ghost()
                    .xsmall()
                    .icon(IconName::Close)
                    .on_click(cx.listener(move |this, _, _window, cx| this.remove_highlight_rule(id, cx))),
            )
            .into_any_element()
    }

    fn render_rule_form(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let form = self.rule_form.as_ref()?;
        let tone = self.tone(cx);
        let muted = cx.theme().muted_foreground;
        let mut card = v_flex()
            .w_full()
            .gap_1p5()
            .p_2()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().border)
            .child(div().text_xs().font_weight(gpui::FontWeight::SEMIBOLD).child("New rule"));

        // Column
        let mut columns = h_flex().w_full().flex_wrap().gap_1();
        for (ix, (key, label)) in self.rule_columns().into_iter().enumerate() {
            let chosen = form.column.as_deref() == Some(key.as_str());
            columns = columns.child(
                Button::new(ElementId::from(format!("hl-form-col-{ix}")))
                    .xsmall()
                    .when(chosen, |b| b.primary())
                    .when(!chosen, |b| b.outline())
                    .label(label)
                    .on_click(cx.listener(move |this, _, _window, cx| this.rule_form_pick_column(&key, cx))),
            );
        }
        card = card.child(columns);

        // Values
        if let Some(key) = form.column.as_deref() {
            if key == MESSAGE_COLUMN {
                card = card.child(div().text_xs().text_color(muted).child("Rows whose message contains the added text."));
            } else if let Some(col) = self.filter_index.column(key) {
                let view = cx.entity();
                // No height cap: the panel scrolls, like the Filter value list.
                let mut list = v_flex().w_full().gap_0p5();
                for (vix, value) in col.values.iter().enumerate() {
                    let display = value.display.clone();
                    let checked = form.values.iter().any(|v| *v == value.display);
                    let view = view.clone();
                    list = list.child(
                        Checkbox::new(ElementId::from(format!("hl-form-val-{vix}")))
                            .label(SharedString::from(format!("{}  ({})", value.display, value.count)))
                            .checked(checked)
                            .on_click(move |_, _window, app| {
                                let display = display.clone();
                                view.update(app, |this, cx| this.rule_form_toggle_value(&display, cx));
                            }),
                    );
                }
                card = card.child(list);
            }
            if !form.custom.is_empty() {
                let mut chips = h_flex().w_full().flex_wrap().gap_1();
                for (cix, text) in form.custom.iter().enumerate() {
                    let value = text.clone();
                    chips = chips.child(
                        Button::new(ElementId::from(format!("hl-form-custom-{cix}")))
                            .outline()
                            .xsmall()
                            .label(format!("contains “{text}”"))
                            .icon(IconName::Close)
                            .on_click(cx.listener(move |this, _, _window, cx| {
                                this.rule_form_remove_custom(&value, cx);
                            })),
                    );
                }
                card = card.child(chips);
            }
            card = card.child(Input::new(&self.rule_input).small());
        }

        // Color
        let mut grid = h_flex().w_full().flex_wrap().gap_1();
        for (cix, color) in palette_cells().into_iter().enumerate() {
            let selected = form.color == color;
            let pick = color.clone();
            grid = grid.child(
                swatch(ElementId::from(format!("hl-form-color-{cix}")), &color, tone, 14.0)
                    .when(selected, |s| s.border_2().border_color(cx.theme().foreground))
                    .on_click(cx.listener(move |this, _, _window, cx| this.rule_form_set_color(pick.clone(), cx))),
            );
        }
        card = card.child(grid);

        if let Some(error) = form.error {
            card = card.child(div().text_xs().text_color(cx.theme().danger).child(error));
        }
        card = card.child(
            h_flex()
                .w_full()
                .gap_2()
                .justify_end()
                .child(
                    Button::new("hl-form-cancel")
                        .ghost()
                        .xsmall()
                        .label("Cancel")
                        .on_click(cx.listener(|this, _, _window, cx| this.cancel_rule_form(cx))),
                )
                .child(
                    Button::new("hl-form-add")
                        .primary()
                        .xsmall()
                        .label("Add")
                        .on_click(cx.listener(|this, _, _window, cx| this.rule_form_submit(cx))),
                ),
        );
        Some(card.into_any_element())
    }

    /// Highlights section body: the rules in priority order and "+ Add rule".
    pub(super) fn render_highlights_panel_content(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let rules = self.highlight_rules();
        let mut root = v_flex().w_full().gap_2();
        root = root.child(
            h_flex().w_full().items_center().justify_between().child(
                Button::new("hl-add-rule")
                    .ghost()
                    .xsmall()
                    .label("+ Add rule")
                    .on_click(cx.listener(|this, _, _window, cx| this.open_rule_form(cx))),
            ),
        );
        if let Some(form) = self.render_rule_form(cx) {
            root = root.child(form);
        }
        for (ix, rule) in rules.iter().enumerate() {
            root = root.child(self.render_rule_row(ix, rule, cx));
        }
        if rules.is_empty() && self.rule_form.is_none() {
            root = root.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("A rule paints every row whose value in a column matches. Right-click a cell to start one."),
            );
        } else if rules.len() > 1 {
            root = root.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("The rule higher in the list wins."),
            );
        }
        root.into_any_element()
    }
}
