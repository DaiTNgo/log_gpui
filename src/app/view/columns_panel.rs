//! UI: Columns section of the context sidebar, the one place that shows and
//! hides the columns of the Focus Pane. Each row is a switch. Columns are
//! listed in groups: *Built-in*, *From Log Format*, *Rules* (Extraction
//! Rules, in priority order) and *Detected*.

use super::AppView;
use crate::app::columns::{ColumnRow, DETECTED_SHOWN, RuleRow};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    AppContext as _, Context, ElementId, InteractiveElement, IntoElement, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::Input;
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_component::switch::Switch;
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::assets::IconName;

/// The payload of dragging an Extraction Rule: its place, its priority.
#[derive(Clone)]
pub struct DragRuleRow {
    pub ix: usize,
    pub label: String,
}

impl Render for DragRuleRow {
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

/// "1,204 lines", or "0 lines here" for a column with no value in the view.
pub fn lines_label(lines: usize) -> String {
    match lines {
        0 => "0 lines here".to_string(),
        1 => "1 line".to_string(),
        n => format!("{} lines", crate::app::format_count(n)),
    }
}

impl AppView {
    pub(crate) fn render_columns_panel_content(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let query = self.columns_query.read(cx).value().to_string();
        let groups = self.column_groups(&query);
        let searching = !query.trim().is_empty() || groups.detected_more > 0;
        let mut body = v_flex().w_full().gap_1();
        body = body.child(group_title("Built-in", cx));
        for row in &groups.built_in {
            body = body.child(self.render_column_row(row, cx));
        }
        if self.active_is_log_space() {
            body = body.child(group_title("From Log Format", cx));
            for row in &groups.from_format {
                body = body.child(self.render_column_row(row, cx));
            }
        }
        if !groups.rules.is_empty() {
            body = body.child(group_title("Rules", cx));
            for rule in &groups.rules {
                body = body.child(self.render_rule_column_row(rule, cx));
            }
        }
        if !groups.detected.is_empty() || searching {
            body = body.child(group_title("Detected", cx));
            if searching {
                body = body.child(div().pb_1().child(Input::new(&self.columns_query).xsmall()));
            }
            for row in &groups.detected {
                body = body.child(self.render_column_row(row, cx));
            }
            if groups.detected_more > 0 {
                body = body.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!(
                            "{} more fields — search to find them (showing {DETECTED_SHOWN})",
                            crate::app::format_count(groups.detected_more)
                        )),
                );
            }
        }
        body.into_any_element()
    }

    fn render_column_row(&self, row: &ColumnRow, cx: &mut Context<Self>) -> gpui::AnyElement {
        let key = row.key.clone();
        let muted = cx.theme().muted_foreground;
        let view = cx.entity();
        let shown = row.shown;
        let switch_key = key.clone();
        h_flex()
            .id(ElementId::from(format!("col-row-{key}")))
            .w_full()
            .gap_2()
            .px_1()
            .py_1()
            .items_center()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().border)
            .opacity(if shown { 1.0 } else { 0.7 })
            .child(
                v_flex()
                    .flex_1()
                    .min_w(px(0.0))
                    .child(div().text_xs().font_weight(gpui::FontWeight::SEMIBOLD).truncate().child(row.name.clone()))
                    .child(div().text_xs().text_color(muted).child(lines_label(row.lines))),
            )
            .when(row.switchable, |d| {
                d.child(
                    Switch::new(ElementId::from(format!("col-switch-{key}")))
                        .checked(shown)
                        .on_click(move |_, _window, app| {
                            let key = switch_key.clone();
                            view.update(app, |this, cx| this.set_column_shown(&key, !shown, cx));
                        }),
                )
            })
            .into_any_element()
    }
}

impl AppView {
    fn render_rule_column_row(&self, rule: &RuleRow, cx: &mut Context<Self>) -> gpui::AnyElement {
        let (id, ix, enabled) = (rule.id, rule.ix, rule.enabled);
        let muted = cx.theme().muted_foreground;
        let view = cx.entity();
        let detail = if enabled {
            format!("{} · {}", rule.description, lines_label(rule.lines))
        } else {
            format!("{} · off", rule.description)
        };
        let name: gpui::AnyElement = v_flex()
            .flex_1()
            .min_w(px(0.0))
            .child(div().text_xs().font_weight(gpui::FontWeight::SEMIBOLD).truncate().child(rule.name.clone()))
            .child(div().text_xs().text_color(muted).truncate().child(detail))
            .into_any_element();
        let switch_view = view.clone();
        let menu_view = view.clone();
        h_flex()
            .id(ElementId::from(format!("col-rule-{id}")))
            .w_full()
            .gap_2()
            .px_1()
            .py_1()
            .items_center()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().border)
            .opacity(if enabled { 1.0 } else { 0.6 })
            .on_drag(DragRuleRow { ix, label: rule.name.clone() }, |drag, _, _, cx| cx.new(|_| drag.clone()))
            .drag_over::<DragRuleRow>(|style, _, _, cx| style.border_color(cx.theme().primary))
            .on_drop(cx.listener(move |this, dragged: &DragRuleRow, _window, cx| {
                this.move_extraction_rule(dragged.ix, ix, cx);
            }))
            .child(div().cursor_grab().text_color(muted).child(Icon::new(IconName::GripVertical).size(px(14.0))))
            .child(name)
            .child(
                Switch::new(ElementId::from(format!("col-rule-switch-{id}")))
                    .checked(enabled)
                    .on_click(move |_, _window, app| {
                        switch_view.update(app, |this, cx| this.set_extraction_rule_enabled(id, !enabled, cx));
                    }),
            )
            .child(
                Button::new(ElementId::from(format!("col-rule-more-{id}")))
                    .ghost()
                    .xsmall()
                    .icon(IconName::Ellipsis)
                    .dropdown_menu(move |menu, _, _| {
                        let (edit_view, delete_view) = (menu_view.clone(), menu_view.clone());
                        menu.item(PopupMenuItem::new("Edit").on_click(move |_, window, app| {
                            let view = edit_view.clone();
                            window.defer(app, move |window, app| {
                                view.update(app, |this, cx| this.open_edit_rule(id, window, cx));
                            });
                        }))
                        .item(PopupMenuItem::separator())
                        .item(PopupMenuItem::new("Delete").on_click(move |_, _, app| {
                            delete_view.update(app, |this, cx| this.remove_extraction_rule(id, cx));
                        }))
                    }),
            )
            .into_any_element()
    }
}

fn group_title(title: &'static str, cx: &Context<AppView>) -> gpui::AnyElement {
    div()
        .pt_2()
        .text_xs()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(cx.theme().muted_foreground)
        .child(title)
        .into_any_element()
}
