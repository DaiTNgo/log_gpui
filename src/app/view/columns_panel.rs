//! UI: Columns section of the context sidebar, the one place that shows and
//! hides the columns of the Focus Pane. Each row is a switch. Columns are
//! listed in groups: *Built-in*, *From Log Format* and *Detected*.

use super::AppView;
use crate::app::columns::{ColumnRow, DETECTED_SHOWN};
use gpui::prelude::FluentBuilder as _;
use gpui::{Context, ElementId, InteractiveElement, IntoElement, ParentElement, Styled, div, px};
use gpui_component::input::Input;
use gpui_component::switch::Switch;
use gpui_component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};

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

fn group_title(title: &'static str, cx: &Context<AppView>) -> gpui::AnyElement {
    div()
        .pt_2()
        .text_xs()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(cx.theme().muted_foreground)
        .child(title)
        .into_any_element()
}
