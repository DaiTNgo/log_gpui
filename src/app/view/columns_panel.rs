//! UI: Columns section of the context sidebar, the one place that shows and
//! hides the columns of the Parsed View, orders the Field Columns and names
//! them. Columns are listed in groups: *Built-in*, *From Log Format* (with
//! the Log Space's Log Format picker), *Rules* (Extraction Rules, in priority
//! order) and *Detected*.

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

/// The payload of dragging a Field Column: its place in the drawing order.
#[derive(Clone)]
pub struct DragColumn {
    pub ix: usize,
    pub label: String,
}

impl Render for DragColumn {
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
            body = body.child(group_title("From Log Format", cx)).child(self.render_space_format_picker(cx));
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
        let highlighted = self.columns_highlighted.as_deref() == Some(key.as_str());
        let editing = self.label_edit.as_ref().filter(|e| e.in_panel && e.key == key);
        let muted = cx.theme().muted_foreground;
        let view = cx.entity();
        let name: gpui::AnyElement = match editing {
            Some(edit) => {
                let cancel_view = view.clone();
                h_flex()
                    .flex_1()
                    .min_w(px(0.0))
                    .gap_1()
                    .on_key_down(move |event: &gpui::KeyDownEvent, _window, cx| {
                        if event.keystroke.key == "escape" {
                            cx.stop_propagation();
                            cancel_view.update(cx, |this, cx| this.cancel_column_label(cx));
                        }
                    })
                    .child(div().flex_1().min_w(px(0.0)).child(Input::new(&edit.input).xsmall()))
                    .when_some(edit.error.clone(), |d, error| {
                        d.child(
                            div()
                                .id(ElementId::from(format!("col-label-error-{key}")))
                                .text_color(cx.theme().danger)
                                .child(Icon::new(IconName::TriangleAlert).size_3())
                                .tooltip(move |window, cx| {
                                    gpui_component::tooltip::Tooltip::new(error.clone()).build(window, cx)
                                }),
                        )
                    })
                    .into_any_element()
            }
            None => v_flex()
                .flex_1()
                .min_w(px(0.0))
                .child(div().text_xs().font_weight(gpui::FontWeight::SEMIBOLD).truncate().child(row.name.clone()))
                .child(div().text_xs().text_color(muted).child(lines_label(row.lines)))
                .into_any_element(),
        };
        let switch_view = view.clone();
        let switch_key = key.clone();
        let shown = row.shown;
        let menu_view = view.clone();
        let menu_key = key.clone();
        let mut el = h_flex()
            .id(ElementId::from(format!("col-row-{key}")))
            .w_full()
            .gap_2()
            .px_1()
            .py_1()
            .items_center()
            .rounded_md()
            .border_1()
            .border_color(if highlighted { cx.theme().primary } else { cx.theme().border })
            .when(highlighted, |d| d.bg(cx.theme().primary.opacity(0.08)))
            .opacity(if shown { 1.0 } else { 0.7 });
        if let Some(ix) = row.order {
            let drag = DragColumn { ix, label: row.name.clone() };
            el = el
                .on_drag(drag, |drag, _, _, cx| cx.new(|_| drag.clone()))
                .drag_over::<DragColumn>(|style, _, _, cx| style.border_color(cx.theme().primary))
                .on_drop(cx.listener(move |this, dragged: &DragColumn, _window, cx| {
                    this.move_field_column(dragged.ix, ix, cx);
                }))
                .child(div().cursor_grab().text_color(muted).child(Icon::new(IconName::GripVertical).size(px(14.0))));
        } else {
            el = el.child(div().w(px(14.0)));
        }
        el.child(name)
            .when(row.switchable, |d| {
                d.child(
                    Switch::new(ElementId::from(format!("col-switch-{key}")))
                        .checked(shown)
                        .on_click(move |_, _window, app| {
                            let key = switch_key.clone();
                            switch_view.update(app, |this, cx| this.set_column_shown(&key, !shown, cx));
                        }),
                )
            })
            .child(
                Button::new(ElementId::from(format!("col-more-{key}")))
                    .ghost()
                    .xsmall()
                    .icon(IconName::Ellipsis)
                    .dropdown_menu(move |menu, _, _| {
                        let (view, key) = (menu_view.clone(), menu_key.clone());
                        menu.item(PopupMenuItem::new("Rename").on_click(move |_, window, app| {
                            let (view, key) = (view.clone(), key.clone());
                            // Deferred so the closing menu cannot steal the input focus.
                            window.defer(app, move |window, app| {
                                view.update(app, |this, cx| this.begin_column_rename(&key, false, window, cx));
                            });
                        }))
                    }),
            )
            .into_any_element()
    }
}

impl AppView {
    /// What the Log Space's choice of Log Format reads as.
    fn format_choice_label(&self, choice: Option<&crate::log_format::FormatChoice>) -> String {
        use crate::log_format::FormatChoice;
        match choice {
            None => "By file name, else Auto-detect".into(),
            Some(FormatChoice::AutoDetect) => "Auto-detect".into(),
            Some(FormatChoice::Format(id)) => {
                self.formats.store.get(id).map_or_else(|| "Auto-detect".into(), |f| f.name.clone())
            }
        }
    }

    /// The picker of the open Log Space's Log Format, atop *From Log Format*.
    fn render_space_format_picker(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let Some(space_id) = self.active_space().map(|s| s.id.clone()) else { return div().into_any_element() };
        let choice = self.space_format_choice(&space_id);
        let label = self.format_choice_label(choice.as_ref());
        let formats: Vec<(String, String)> = self.log_formats().iter().map(|f| (f.id.clone(), f.name.clone())).collect();
        let view = cx.entity();
        h_flex()
            .w_full()
            .child(
                Button::new("columns-format-picker")
                    .small()
                    .w_full()
                    .label(label)
                    .icon(IconName::ChevronDown)
                    .dropdown_menu(move |menu, _, _| crate::ui::space_menu::format_menu(menu, &view, &space_id, None, choice.clone(), &formats)),
            )
            .into_any_element()
    }

    fn render_rule_column_row(&self, rule: &RuleRow, cx: &mut Context<Self>) -> gpui::AnyElement {
        let (id, ix, enabled) = (rule.id, rule.ix, rule.enabled);
        let muted = cx.theme().muted_foreground;
        let view = cx.entity();
        let field = self.extraction_rules().get(ix).map(|r| r.name.clone()).unwrap_or_default();
        let editing = self.label_edit.as_ref().filter(|e| e.in_panel && e.key == field);
        let detail = if enabled {
            format!("{} · {}", rule.description, lines_label(rule.lines))
        } else {
            format!("{} · off", rule.description)
        };
        let name: gpui::AnyElement = match editing {
            Some(edit) => div().flex_1().min_w(px(0.0)).child(Input::new(&edit.input).xsmall()).into_any_element(),
            None => v_flex()
                .flex_1()
                .min_w(px(0.0))
                .child(div().text_xs().font_weight(gpui::FontWeight::SEMIBOLD).truncate().child(rule.name.clone()))
                .child(div().text_xs().text_color(muted).truncate().child(detail))
                .into_any_element(),
        };
        let highlighted = self.columns_highlighted.as_deref() == Some(field.as_str());
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
            .border_color(if highlighted { cx.theme().primary } else { cx.theme().border })
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
                        let (edit_view, rename_view, delete_view) = (menu_view.clone(), menu_view.clone(), menu_view.clone());
                        let field = field.clone();
                        menu.item(PopupMenuItem::new("Edit").on_click(move |_, window, app| {
                            let view = edit_view.clone();
                            window.defer(app, move |window, app| {
                                view.update(app, |this, cx| this.open_edit_rule(id, window, cx));
                            });
                        }))
                        .item(PopupMenuItem::new("Rename").on_click(move |_, window, app| {
                            let (view, key) = (rename_view.clone(), field.clone());
                            window.defer(app, move |window, app| {
                                view.update(app, |this, cx| this.begin_column_rename(&key, false, window, cx));
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
