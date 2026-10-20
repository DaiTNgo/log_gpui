//! Context menu of a cell in the Parsed View: color commands and "Make
//! column…". There is no command to filter from here. Also the menu of the
//! Focus Pane's lines.

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
    if let Some((row, sel)) = this.make_column_target() {
        let make_view = view.clone();
        menu = menu.item(PopupMenuItem::separator()).item(PopupMenuItem::new("Make column…").on_click(
            move |_, window, app| {
                let (view, sel) = (make_view.clone(), sel.clone());
                // Deferred so the closing menu cannot steal the input focus.
                window.defer(app, move |window, app| {
                    view.update(app, |this, cx| this.open_new_column(row, sel, window, cx));
                });
            },
        ));
    }
    if let Some((column, value)) = target.column_value.clone() {
        let rule_view = view.clone();
        let label = if column == crate::highlight::MESSAGE_COLUMN {
            format!("Highlight rows where message contains “{value}”")
        } else {
            format!("Highlight rows where {} = {value}", this.column_display_name(&column))
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

/// Menu of the Focus Pane's lines: "Make column…" from text selected in one
/// line.
pub fn pane_menu(menu: PopupMenu, view: &Entity<AppView>, cx: &App) -> PopupMenu {
    let target = view.read(cx).pane_make_column_target();
    let make_view = view.clone();
    menu.item(PopupMenuItem::new("Make column…").disabled(target.is_none()).on_click(move |_, window, app| {
        let Some((row, sel)) = target.clone() else { return };
        let view = make_view.clone();
        window.defer(app, move |window, app| {
            view.update(app, |this, cx| this.open_new_column(row, sel, window, cx));
        });
    }))
}
