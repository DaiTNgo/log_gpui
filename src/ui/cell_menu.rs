//! Context menu of a cell in the Parsed View: color commands and "Log
//! Format…" of the cell's file. There is no command to filter from here.
//! Also the menu of the Focus Pane's lines.

use crate::app::AppView;
use gpui::{App, Entity};
use gpui_component::menu::{PopupMenu, PopupMenuItem};

pub fn cell_menu(menu: PopupMenu, view: &Entity<AppView>, cx: &App) -> PopupMenu {
    let this = view.read(cx);
    let Some(target) = this.cell_target.clone() else { return menu };
    let has_selection = this.text_selection.is_some_and(|s| !s.is_empty());
    let marker = this.marker_under(target.row, target.byte);

    let mut menu = menu;
    let mark_view = view.clone();
    let at = target.at;
    menu = menu.item(
        PopupMenuItem::new("Mark with color")
            .disabled(!has_selection)
            .on_click(move |_, _, app| {
                mark_view.update(app, |this, cx| this.open_mark_bar(at, cx));
            }),
    );
    if marker.is_some() {
        let (row, byte) = (target.row, target.byte);
        let change_view = view.clone();
        let remove_view = view.clone();
        menu = menu
            .item(PopupMenuItem::new("Change color").on_click(move |_, _, app| {
                change_view.update(app, |this, cx| this.open_recolor_bar(row, byte, at, cx));
            }))
            .item(PopupMenuItem::new("Remove marker").on_click(move |_, _, app| {
                remove_view.update(app, |this, cx| this.remove_marker_under(row, byte, cx));
            }));
    }
    if this.log_format_target_of_row(target.row).is_some() {
        let row = target.row;
        let format_view = view.clone();
        menu = menu.item(PopupMenuItem::separator()).item(PopupMenuItem::new("Log Format…").on_click(
            move |_, window, app| {
                let view = format_view.clone();
                // Deferred so the closing menu cannot steal the input focus.
                window.defer(app, move |window, app| {
                    view.update(app, |this, cx| this.edit_log_format_of_row(row, window, cx));
                });
            },
        ));
    }
    if let Some((column, value)) = target.column_value.clone() {
        let rule_view = view.clone();
        let label = if column == crate::highlight::MESSAGE_COLUMN {
            format!("Highlight rows where message contains “{value}”")
        } else {
            format!("Highlight rows where {} = {value}", crate::columns::default_name(&column))
        };
        menu = menu.item(PopupMenuItem::separator()).item(PopupMenuItem::new(label).on_click(
            move |_, _, app| {
                let (column, value) = (column.clone(), value.clone());
                rule_view.update(app, |this, cx| this.highlight_rows_where(&column, &value, cx));
            },
        ));
    }
    menu
}

/// Menu of the Focus Pane's lines: "Log Format…" for the line selected (or
/// the one selected text is in).
pub fn pane_menu(menu: PopupMenu, view: &Entity<AppView>, cx: &App) -> PopupMenu {
    let target = view.read(cx).pane_log_format_row();
    let format_view = view.clone();
    menu.item(PopupMenuItem::new("Log Format…").disabled(target.is_none()).on_click(move |_, window, app| {
        let Some(row) = target else { return };
        let view = format_view.clone();
        window.defer(app, move |window, app| {
            view.update(app, |this, cx| this.edit_log_format_of_row(row, window, cx));
        });
    }))
}
