//! UI: Filter section of the context sidebar.
//!
//! Columns and their values come from the loaded file. Per column the user
//! ticks values, adds custom values or hides the chosen values; `timestamp`
//! has its own Time Range.

use super::AppView;
use crate::filter_index::{ColumnIndex, MAX_DISTINCT};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    div, px, Context, ElementId, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::checkbox::Checkbox;
use gpui_component::input::Input;
use gpui_component::tag::Tag;
use gpui_component::{h_flex, v_flex, ActiveTheme as _, Icon, Selectable as _, Sizable as _};
use gpui_kit::assets::IconName;

impl AppView {
    /// The name Filter shows for a column: `fallback`.
    fn filter_column_name(&self, _key: &str, fallback: &str) -> String {
        fallback.to_string()
    }

    /// Keys of the columns shown as cards: those with a condition, plus the
    /// column being edited. Conditions on a column the file does not have are
    /// listed too, so the user can clear them.
    fn filter_card_keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = Vec::new();
        for col in self.filter_index.columns_in_use() {
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
                && self.filter_index.column(&cf.column).is_none()
            {
                keys.push(cf.column.clone());
            }
        }
        keys
    }

    fn render_column_picker(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let mut list = v_flex().w_full().gap_0p5().py_1();
        let mut any = false;
        for (ix, col) in self.filter_index.columns_in_use().enumerate() {
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
        if !any {
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
        let mut list = v_flex().w_full().gap_0p5();
        for (vix, value) in col.values.iter().enumerate() {
            let key = col.key.clone();
            let folded = value.folded.clone();
            let checked = self.filter_state.is_value_ticked(&col.key, &value.folded);
            let view = view.clone();
            list = list.child(
                Checkbox::new(ElementId::from(format!("filter-val-{}-{}", col.key, vix)))
                    .label(SharedString::from(format!("{}  ({})", value.display, value.count)))
                    .checked(checked)
                    .on_click(move |_, _window, app| {
                        let (key, folded) = (key.clone(), folded.clone());
                        view.update(app, |this, cx| this.filter_toggle_value(&key, &folded, cx));
                    }),
            );
        }
        // A ticked value whose last line left the Live Window stays, with 0.
        let missing: Vec<String> = self
            .filter_state
            .column(&col.key)
            .map(|c| {
                c.values
                    .iter()
                    .filter(|v| !col.values.iter().any(|x| &x.folded == *v))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        for (i, folded) in missing.into_iter().enumerate() {
            let key = col.key.clone();
            let view = view.clone();
            let toggled = folded.clone();
            list = list.child(
                Checkbox::new(ElementId::from(format!("filter-val-{}-missing-{}", col.key, i)))
                    .label(SharedString::from(format!("{folded}  (0)")))
                    .checked(true)
                    .on_click(move |_, _window, app| {
                        let (key, folded) = (key.clone(), toggled.clone());
                        view.update(app, |this, cx| this.filter_toggle_value(&key, &folded, cx));
                    }),
            );
        }
        list.into_any_element()
    }

    fn render_filter_card(&self, ix: usize, key: &str, cx: &mut Context<Self>) -> gpui::AnyElement {
        let col = self.filter_index.column(key);
        let label = self.filter_column_name(key, &col.map_or_else(|| key.to_string(), |c| c.label.clone()));
        let cf = self.filter_state.column(key);
        let hide = cf.is_some_and(|c| c.hide);
        let count = cf.map_or(0, |c| c.values.len() + c.custom.len());
        let expanded = self.filter_ui.expanded.as_deref() == Some(key);

        let toggle_key = key.to_string();
        let clear_key = key.to_string();
        let header = h_flex()
            .w_full()
            .gap_2()
            .items_center()
            .child(
                h_flex()
                    .id(ElementId::from(format!("filter-card-{}", ix)))
                    .flex_1()
                    .min_w(px(0.0))
                    .gap_2()
                    .items_center()
                    .cursor_pointer()
                    .child(
                        Icon::new(if expanded { IconName::ChevronDown } else { IconName::ChevronRight })
                            .size_3()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(label),
                    )
                    .when(hide && count > 0, |this| this.child(Tag::secondary().child("hide")))
                    .when(count > 0, |this| this.child(Tag::secondary().child(count.to_string())))
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
            .p_2()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().border)
            .child(header)
            .when_some(col.is_none().then(|| self.unread_column(key)).flatten(), |card, reason| {
                let note = match reason {
                    crate::app::rules::UnreadReason::NotRead => "column no longer read".to_string(),
                    other => format!("column no longer read ({})", other.text()),
                };
                card.child(div().text_xs().text_color(cx.theme().muted_foreground).child(note))
            });

        if expanded {
            let view = cx.entity();
            let hide_key = key.to_string();
            card = card.child(
                Checkbox::new(ElementId::from(format!("filter-hide-{}", ix)))
                    .label("Hide the chosen values")
                    .checked(hide)
                    .on_click(move |checked, _window, app| {
                        let (key, checked) = (hide_key.clone(), *checked);
                        view.update(app, |this, cx| this.filter_set_hide(&key, checked, cx));
                    }),
            );
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
            if col.is_none_or(|c| c.is_filterable()) {
                card = card.child(Input::new(&self.filter_ui.value_input).small());
            }
        }
        card.into_any_element()
    }

    fn render_time_range(&self) -> gpui::AnyElement {
        let from_ok = self.time_bound_is_valid(&self.filter_state.time_range.from);
        let to_ok = self.time_bound_is_valid(&self.filter_state.time_range.to);
        v_flex()
            .w_full()
            .gap_1()
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Time range"),
                    )
                    .when(!from_ok || !to_ok, |this| this.child(Tag::danger().child("! invalid time"))),
            )
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .child(div().flex_1().min_w(px(0.0)).child(Input::new(&self.filter_ui.from_input).small()))
                    .child(div().flex_1().min_w(px(0.0)).child(Input::new(&self.filter_ui.to_input).small())),
            )
            .into_any_element()
    }

    /// Filter section body: add filter, one card per filtered column, time range.
    pub(super) fn render_filter_panel_content(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let mut root = v_flex().w_full().gap_2();

        root = root.child(
            h_flex()
                .w_full()
                .items_center()
                .justify_between()
                .child(
                    Button::new("filter-add")
                        .ghost()
                        .xsmall()
                        .label("+ Add filter")
                        .selected(self.filter_ui.picker_open)
                        .on_click(cx.listener(|this, _, _window, cx| this.toggle_filter_picker(cx))),
                )
                .when(!self.filter_state.is_empty(), |this| {
                    this.child(
                        Button::new("filter-clear-all")
                            .ghost()
                            .xsmall()
                            .label("Clear all")
                            .on_click(cx.listener(|this, _, _window, cx| this.filter_clear_all(cx))),
                    )
                }),
        );

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
                    .child("Add a filter to choose which values to show or hide."),
            );
        }

        root.child(self.render_time_range()).into_any_element()
    }
}
