//! UI: Filter section of the context sidebar.
//!
//! Columns and their values come from the loaded file. Per column the user
//! ticks values, adds custom values or hides the chosen values; `timestamp`
//! has its own Time Range. Message has no values to tick: only custom
//! values, matched against the text of each line's message.

use super::AppView;
use crate::columns::MESSAGE_KEY;
use crate::filter_index::{ColumnIndex, MAX_DISTINCT};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    div, px, Context, ElementId, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled,
};
use gpui_component::button::{Button, ButtonGroup, ButtonVariants as _};
use gpui_component::checkbox::Checkbox;
use gpui_component::input::Input;
use gpui_component::tag::Tag;
use gpui_component::{h_flex, v_flex, ActiveTheme as _, Icon, Selectable as _, Sizable as _};
use gpui_kit::assets::IconName;

/// Tallest the value list of an open filter card grows before it scrolls.
const VALUE_LIST_MAX_HEIGHT: f32 = 220.0;
/// From this many values the list always takes the full capped height.
const VALUE_LIST_FIXED_FROM: usize = 12;

impl AppView {
    /// The name Filter shows for a column: `fallback`.
    fn filter_column_name(&self, key: &str, fallback: &str) -> String {
        if key == MESSAGE_KEY { "Message".to_string() } else { fallback.to_string() }
    }

    /// Keys of the columns shown as cards: those with a condition, plus the
    /// column being edited. Conditions on a column the file does not have are
    /// listed too, so the user can clear them.
    fn filter_card_keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = Vec::new();
        for col in self.filter_index.read(&self.view.rows).columns_in_use() {
            let active = self
                .filter_state
                .column(&col.key)
                .is_some_and(|c| c.is_active() || c.hide);
            if active || self.filter_ui.expanded.as_deref() == Some(col.key.as_str()) {
                keys.push(col.key.clone());
            }
        }
        for cf in &self.filter_state.columns {
            if (cf.is_active() || cf.hide)
                && !keys.contains(&cf.column)
                && self.filter_index.read(&self.view.rows).column(&cf.column).is_none()
            {
                keys.push(cf.column.clone());
            }
        }
        if self.filter_ui.expanded.as_deref() == Some(MESSAGE_KEY) && !keys.iter().any(|k| k == MESSAGE_KEY) {
            keys.push(MESSAGE_KEY.to_string());
        }
        keys
    }

    fn render_column_picker(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let mut list = v_flex().w_full().gap_0p5().py_1();
        let mut any = false;
        for (ix, col) in self.filter_index.read(&self.view.rows).columns_in_use().enumerate() {
            any = true;
            let key = col.key.clone();
            let available = col.is_filterable();
            let row = h_flex()
                .id(ElementId::from(format!("filter-pick-{}", ix)))
                .w_full()
                .px_2()
                .py_1()
                .gap_2()
                .items_center()
                .justify_between()
                .rounded_md()
                .child(div().text_xs().child(self.filter_column_name(&col.key, &col.label)))
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(if available {
                            format!("{} values", col.distinct_label())
                        } else {
                            format!("{} values · too many to filter", col.distinct_label())
                        }),
                );
            list = list.child(if available {
                row.cursor_pointer()
                    .hover(|s| s.bg(cx.theme().accent))
                    .on_click(cx.listener(move |this, _, _window, cx| {
                        this.pick_filter_column(&key, cx);
                    }))
                    .into_any_element()
            } else {
                row.opacity(0.5).into_any_element()
            });
        }
        if any {
            list = list.child(
                h_flex()
                    .id("filter-pick-message")
                    .debug_selector(|| "filter-pick-message".into())
                    .w_full()
                    .px_2()
                    .py_1()
                    .gap_2()
                    .items_center()
                    .justify_between()
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|s| s.bg(cx.theme().accent))
                    .child(div().text_xs().child(self.filter_column_name(MESSAGE_KEY, MESSAGE_KEY)))
                    .child(div().text_xs().text_color(cx.theme().muted_foreground).child("text it contains"))
                    .on_click(cx.listener(|this, _, _window, cx| this.pick_filter_column(MESSAGE_KEY, cx))),
            );
        } else {
            list = list.child(
                div()
                    .px_2()
                    .py_1()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("No columns detected in this file."),
            );
        }
        list.into_any_element()
    }

    fn render_value_list(&self, col: &ColumnIndex, cx: &mut Context<Self>) -> gpui::AnyElement {
        let view = cx.entity();
        let query = crate::unicode_text::fold(self.filter_ui.search_input.read(cx).value().trim());
        let mut list = v_flex().w_full().gap_0p5();
        for (vix, value) in col.values.iter().enumerate() {
            if !query.is_empty() && !value.folded.contains(&query) {
                continue;
            }
            let key = col.key.clone();
            let folded = value.folded.clone();
            let checked = self.filter_state.is_value_ticked(&col.key, &value.folded);
            let view = view.clone();
            list = list.child(value_row(
                Checkbox::new(ElementId::from(format!("filter-val-{}-{}", col.key, vix)))
                    .label(SharedString::from(value.display.clone()))
                    .checked(checked)
                    .on_click(move |_, _window, app| {
                        let (key, folded) = (key.clone(), folded.clone());
                        view.update(app, |this, cx| this.filter_toggle_value(&key, &folded, cx));
                    }),
                value.count,
                cx,
            ));
        }
        // A ticked value whose last line left the Live Window stays, with 0.
        let missing: Vec<String> = self
            .filter_state
            .column(&col.key)
            .map(|c| {
                c.values
                    .iter()
                    .filter(|v| query.is_empty() || v.contains(&query))
                    .filter(|v| !col.values.iter().any(|x| &x.folded == *v))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        for (i, folded) in missing.into_iter().enumerate() {
            let key = col.key.clone();
            let view = view.clone();
            let toggled = folded.clone();
            list = list.child(value_row(
                Checkbox::new(ElementId::from(format!("filter-val-{}-missing-{}", col.key, i)))
                    .label(SharedString::from(folded))
                    .checked(true)
                    .on_click(move |_, _window, app| {
                        let (key, folded) = (key.clone(), toggled.clone());
                        view.update(app, |this, cx| this.filter_toggle_value(&key, &folded, cx));
                    }),
                0,
                cx,
            ));
        }
        let many = col.values.len() >= VALUE_LIST_FIXED_FROM;
        // Capped, and fixed once the list is long: ticking or searching must
        // not push the sections below it around.
        let scroller = div()
            .id(ElementId::from(format!("filter-vals-{}", col.key)))
            .w_full()
            .when(many, |d| d.h(px(VALUE_LIST_MAX_HEIGHT)))
            .max_h(px(VALUE_LIST_MAX_HEIGHT))
            .overflow_y_scroll()
            .child(list)
            .when(!query.is_empty() && self.value_list_is_empty(col, &query), |d| {
                d.child(div().text_xs().text_color(cx.theme().muted_foreground).child("No value matches."))
            });
        scroller.into_any_element()
    }

    fn value_list_is_empty(&self, col: &ColumnIndex, query: &str) -> bool {
        !col.values.iter().any(|v| v.folded.contains(query))
            && !self
                .filter_state
                .column(&col.key)
                .is_some_and(|c| c.values.iter().any(|v| v.contains(query)))
    }

    fn render_filter_card(&self, ix: usize, key: &str, cx: &mut Context<Self>) -> gpui::AnyElement {
        let index = self.filter_index.read(&self.view.rows);
        let col = index.column(key);
        let label = self.filter_column_name(key, &col.map_or_else(|| key.to_string(), |c| c.label.clone()));
        let cf = self.filter_state.column(key);
        let hide = cf.is_some_and(|c| c.hide);
        let expanded = self.filter_ui.expanded.as_deref() == Some(key);
        let summary = cf.filter(|c| c.is_active()).map(|c| {
            let text = c.chip_text("", |v| {
                col.and_then(|col| col.values.iter().find(|x| x.folded == v)).map_or_else(|| v.to_string(), |x| x.display.clone())
            });
            text.trim_start_matches("not ").trim_start_matches(": ").to_string()
        });

        let toggle_key = key.to_string();
        let clear_key = key.to_string();
        let header = h_flex()
            .w_full()
            .h(px(24.0))
            .gap_2()
            .items_center()
            .child(
                h_flex()
                    .id(ElementId::from(format!("filter-card-{}", ix)))
                    .flex_1()
                    .min_w(px(0.0))
                    .gap_1p5()
                    .items_center()
                    .cursor_pointer()
                    .child(
                        Icon::new(if expanded { IconName::ChevronDown } else { IconName::ChevronRight })
                            .size_3()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .when(hide && summary.is_some(), |this| {
                        this.child(div().text_xs().text_color(cx.theme().danger).child("not"))
                    })
                    .child(div().flex_shrink_0().text_xs().child(label))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .truncate()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .when(self.filter_state.paused, |d| d.line_through())
                            .children(summary),
                    )
                    .on_click(cx.listener(move |this, _, _window, cx| {
                        this.toggle_filter_column(&toggle_key, cx);
                    })),
            )
            .child(
                Button::new(ElementId::from(format!("filter-card-clear-{}", ix)))
                    .ghost()
                    .xsmall()
                    .icon(IconName::Close)
                    .on_click(cx.listener(move |this, _, _window, cx| {
                        this.filter_clear_column(&clear_key, cx);
                    })),
            );

        let mut card = v_flex()
            .w_full()
            .gap_1p5()
            .px_1()
            .rounded_md()
            .when(expanded, |c| c.pb_2().bg(cx.theme().secondary))
            .child(header)
            .when_some(col.is_none().then(|| self.unread_column(key)).flatten(), |card, reason| {
                let note = match reason {
                    crate::app::line_config::UnreadReason::NotRead => "column no longer read".to_string(),
                    other => format!("column no longer read ({})", other.text()),
                };
                card.child(div().text_xs().text_color(cx.theme().muted_foreground).child(note))
            });

        if expanded {
            if col.is_none_or(|c| c.is_filterable()) {
                card = card.child(Input::new(&self.filter_ui.search_input).small());
            }
            match col {
                Some(col) if col.is_filterable() => {
                    card = card.child(self.render_value_list(col, cx));
                }
                Some(_) => {
                    card = card.child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("More than {} values: no new conditions.", MAX_DISTINCT)),
                    );
                }
                None if key == MESSAGE_KEY => {
                    card = card.child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("Lines whose message contains the text added (Enter)."),
                    );
                }
                None => {
                    card = card.child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("This column is not in the current file."),
                    );
                }
            }
            if let Some(cf) = cf
                && !cf.custom.is_empty()
            {
                let mut chips = h_flex().w_full().flex_wrap().gap_1();
                for (cix, text) in cf.custom.iter().enumerate() {
                    let (key, value) = (key.to_string(), text.clone());
                    chips = chips.child(
                        Button::new(ElementId::from(format!("filter-custom-{}-{}", ix, cix)))
                            .outline()
                            .xsmall()
                            .label(format!("contains “{}”", text))
                            .icon(IconName::Close)
                            .on_click(cx.listener(move |this, _, _window, cx| {
                                this.filter_remove_custom(&key, &value, cx);
                            })),
                    );
                }
                card = card.child(chips);
            }
            let (show_key, hide_key) = (key.to_string(), key.to_string());
            card = card.child(
                ButtonGroup::new(ElementId::from(format!("filter-mode-{}", ix)))
                    .outline()
                    .xsmall()
                    .child(Button::new(ElementId::from(format!("filter-show-{}", ix))).label("Show only these").selected(!hide))
                    .child(Button::new(ElementId::from(format!("filter-hide-{}", ix))).label("Hide these").selected(hide))
                    .on_click(cx.listener(move |this, selected: &Vec<usize>, _, cx| {
                        let hide = selected.first() == Some(&1);
                        let key = if hide { &hide_key } else { &show_key };
                        this.filter_set_hide(key, hide, cx);
                    })),
            );
        }
        card.into_any_element()
    }

    /// The Time Range: From and To, one above the other.
    fn render_time_range(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let range = &self.filter_state.time_range;
        let valid = self.time_bound_is_valid(&range.from) && self.time_bound_is_valid(&range.to);
        let field = |label: &'static str, input| {
            v_flex()
                .w_full()
                .gap_0p5()
                .child(div().text_xs().text_color(cx.theme().muted_foreground).child(label))
                .child(Input::new(input).small())
        };
        v_flex()
            .w_full()
            .gap_1()
            .child(field("From", &self.filter_ui.from_input))
            .child(field("To", &self.filter_ui.to_input))
            .when(!valid, |this| this.child(Tag::danger().child("! invalid time")))
            .into_any_element()
    }

    /// Filter section body: add filter, one card per filtered column, time range.
    pub(super) fn render_filter_panel_content(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let mut root = v_flex().w_full().gap_2();

        if let Some(percent) = self.filter_read_so_far() {
            root = root.child(
                div()
                    .id("filter-read-so-far")
                    .debug_selector(|| "filter-read-so-far".into())
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("Still reading ({percent}%): values and counts so far.")),
            );
        }

        if let Some((scanned, total)) = self.filter_scanning {
            let percent = (scanned * 100).checked_div(total).unwrap_or(0);
            root = root.child(
                div()
                    .id("filter-scanning")
                    .debug_selector(|| "filter-scanning".into())
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("Reading the lines ({percent}%): the lines found so far show.")),
            );
        }

        if self.filter_ui.picker_open {
            root = root.child(self.render_column_picker(cx));
        }

        for (ix, key) in self.filter_card_keys().into_iter().enumerate() {
            root = root.child(self.render_filter_card(ix, &key, cx));
        }

        if self.filter_state.is_empty() && !self.filter_ui.picker_open && self.filter_ui.expanded.is_none() {
            root = root.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("Press + to choose which values to show or hide, or right-click a cell."),
            );
        }

        root.child(self.render_time_range(cx)).into_any_element()
    }
}

/// A value of the list: its checkbox, and its number of lines on the right.
fn value_row(checkbox: Checkbox, count: usize, cx: &gpui::App) -> gpui::AnyElement {
    h_flex()
        .w_full()
        .gap_2()
        .items_center()
        .child(div().flex_1().min_w(px(0.0)).child(checkbox))
        .child(div().flex_shrink_0().text_xs().text_color(cx.theme().muted_foreground).child(crate::app::format_count(count)))
        .into_any_element()
}
