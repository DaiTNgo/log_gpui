//! UI: Highlights section of the context sidebar. Each Highlight Rule is a
//! row with its color, column, values, how many rows it matches, a switch, a
//! delete button and a handle to drag it up or down the priority list.

use super::AppView;
use crate::highlight::{HighlightRule, MESSAGE_COLUMN};
use crate::palette::{HighlightColor, Tone};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    AppContext as _, Context, ElementId, InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::checkbox::Checkbox;
use gpui_component::input::Input;
use gpui_component::switch::Switch;
use gpui_component::menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenuItem};
use gpui_component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
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
        let (menu_view, more_view) = (cx.entity(), cx.entity());
        let enabled = rule.enabled;
        let group = SharedString::from(format!("hl-rule-{id}"));
        h_flex()
            .id(ElementId::from(format!("hl-rule-{id}")))
            .group(group.clone())
            .cursor_grab()
            .w_full()
            .h(px(28.0))
            .gap_2()
            .px_1()
            .items_center()
            .rounded_md()
            .hover(|s| s.bg(cx.theme().secondary))
            .opacity(if enabled { 1.0 } else { 0.55 })
            .on_drag(drag, |drag, _, _, cx| cx.new(|_| drag.clone()))
            .drag_over::<DragRule>(|style, _, _, cx| style.bg(cx.theme().accent))
            .on_drop(cx.listener(move |this, dragged: &DragRule, _window, cx| {
                this.move_highlight_rule(dragged.ix, ix, cx);
            }))
            .child(
                swatch(ElementId::from(format!("hl-color-{id}")), &rule.color, tone, 14.0).on_click(
                    cx.listener(move |this, event: &gpui::ClickEvent, _window, cx| {
                        this.open_rule_palette(id, event.position(), cx);
                    }),
                ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .truncate()
                    .text_xs()
                    .child(rule_condition(&column, rule)),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(super::columns_panel::lines_label(matches)),
            )
            .child(
                Switch::new(ElementId::from(format!("hl-switch-{id}")))
                    .small()
                    .checked(enabled)
                    .on_click(move |_, _window, app| {
                        view.update(app, |this, cx| this.toggle_highlight_rule(id, cx));
                    }),
            )
            .child(
                div().invisible().group_hover(group, |s| s.visible()).child(
                    Button::new(ElementId::from(format!("hl-more-{id}")))
                        .ghost()
                        .xsmall()
                        .icon(IconName::Ellipsis)
                        .dropdown_menu(move |menu, _, _| rule_menu(menu, id, &more_view)),
                ),
            )
            .context_menu(move |menu, _, _| rule_menu(menu, id, &menu_view))
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
            card = card.child(Input::new(&self.rule_search).small());
            if key == MESSAGE_COLUMN {
                card = card.child(div().text_xs().text_color(muted).child("Rows whose message contains the text added (Enter)."));
            } else if let Some(col) = self.filter_index.column(key).filter(|c| c.is_filterable()) {
                let view = cx.entity();
                let query = crate::unicode_text::fold(self.rule_search.read(cx).value().trim());
                // Capped so ticking values leaves the rest of the sidebar where it is.
                let mut list = v_flex().w_full().gap_0p5();
                let mut shown = 0;
                for (vix, value) in col.values.iter().enumerate() {
                    if !query.is_empty() && !value.folded.contains(&query) {
                        continue;
                    }
                    shown += 1;
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
                if shown == 0 {
                    list = list.child(div().text_xs().text_color(muted).child("No value matches."));
                }
                let many = col.values.len() >= 12;
                card = card.child(
                    div()
                        .id("hl-form-values")
                        .w_full()
                        .when(many, |d| d.h(px(220.0)))
                        .max_h(px(220.0))
                        .overflow_y_scroll()
                        .child(list),
                );
            } else if self.filter_index.column(key).is_some() {
                card = card.child(div().text_xs().text_color(muted).child(format!(
                    "More than {} values: add text the value contains.",
                    crate::filter_index::MAX_DISTINCT
                )));
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
        }

        // Color: eight distinct ones, the chosen one, and the whole palette.
        let mut colors = rule_form_colors();
        if !colors.contains(&form.color) {
            colors.push(form.color.clone());
        }
        let mut grid = h_flex().w_full().flex_wrap().gap_1().items_center();
        for (cix, color) in colors.into_iter().enumerate() {
            let selected = form.color == color;
            let pick = color.clone();
            grid = grid.child(
                swatch(ElementId::from(format!("hl-form-color-{cix}")), &color, tone, 18.0)
                    .when(selected, |s| s.border_2().border_color(cx.theme().foreground))
                    .on_click(cx.listener(move |this, _, _window, cx| this.rule_form_set_color(pick.clone(), cx))),
            );
        }
        grid = grid.child(
            Button::new("hl-form-more-colors")
                .ghost()
                .xsmall()
                .label("…")
                .tooltip("All colors")
                .on_click(cx.listener(|this, event: &gpui::ClickEvent, _window, cx| {
                    this.open_rule_form_palette(event.position(), cx)
                })),
        );
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
                    .child("A rule paints every row whose value in a column matches. Press + or right-click a cell to start one."),
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

/// The menu of a rule's row.
fn rule_menu(menu: gpui_component::menu::PopupMenu, id: u64, view: &gpui::Entity<AppView>) -> gpui_component::menu::PopupMenu {
    let view = view.clone();
    menu.item(
        PopupMenuItem::new("Delete rule")
            .on_click(move |_, _, app| view.update(app, |this, cx| this.remove_highlight_rule(id, cx))),
    )
}

/// The colors the New rule form offers at once: the medium shade of eight
/// hues far apart.
pub fn rule_form_colors() -> Vec<HighlightColor> {
    use crate::palette::{Hue, Shade};
    [Hue::Red, Hue::Orange, Hue::Amber, Hue::Green, Hue::Teal, Hue::Blue, Hue::Purple, Hue::Pink]
        .into_iter()
        .map(|h| HighlightColor::palette(h, Shade::Medium))
        .collect()
}

/// A rule's condition in one line: `Level = info, warn`,
/// `Message contains "timeout"`.
fn rule_condition(column: &str, rule: &HighlightRule) -> String {
    let mut parts: Vec<String> = rule.values.iter().cloned().collect();
    parts.extend(rule.custom.iter().map(|c| format!("contains “{c}”")));
    if rule.values.is_empty() {
        format!("{column} {}", parts.join(", "))
    } else {
        format!("{column} = {}", parts.join(", "))
    }
}
