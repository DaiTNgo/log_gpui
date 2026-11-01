//! The Selection Menu: the right-click menu on text in the Parsed View, the
//! Raw View list and a scratch editor. Copy; "Mark here" and "Highlight
//! everywhere" as rows of colors; the color bar; Show only / Hide; a rule
//! from the cell; and removing the Marker or Text Highlight under the pointer.

use crate::app::AppView;
use crate::app::color::short_text;
use crate::app::selection_menu::{CellValue, MenuSource, SelectionTarget};
use crate::keymap::Command;
use crate::palette::{HighlightColor, Tone};
use gpui::{App, Entity, InteractiveElement as _, ParentElement as _, StatefulInteractiveElement as _, Styled as _, div, px};
use gpui_component::kbd::Kbd;
use gpui_component::menu::{PopupMenu, PopupMenuItem};
use gpui_component::{ActiveTheme as _, h_flex};

/// Length a selected text is cut to in a label.
const LABEL_CHARS: usize = 24;

pub fn selection_menu(menu: PopupMenu, view: &Entity<AppView>, cx: &App) -> PopupMenu {
    let this = view.read(cx);
    let Some(target) = this.selection_menu.clone() else { return menu };
    let colors = this.bar_colors();
    let tone = if cx.theme().is_dark() { Tone::Dark } else { Tone::Light };
    let color_keys = this.keymap.shortcuts(Command::ColorSelection).first().map(|s| s.keystroke());
    let filter_custom = target.cell.as_ref().map(|c| this.filters_as_custom(&c.column, c.whole));

    let mut menu = menu.min_w(px(240.0));
    let mut group = false;
    if target.text.is_some() {
        let v = view.clone();
        menu = menu.item(PopupMenuItem::new("Copy").on_click(move |_, _, app| v.update(app, |this, cx| this.menu_copy(cx))));
        group = true;
    }
    if target.mark.is_some() {
        menu = separate(menu, &mut group);
        menu = menu.item(swatch_row("sel-mark", "Mark here", &colors, tone, view, |this, color, cx| {
            this.menu_mark_here(color, cx)
        }));
        if target.everywhere.is_some() {
            menu = menu.item(swatch_row("sel-everywhere", "Highlight everywhere", &colors, tone, view, |this, color, cx| {
                this.menu_highlight_everywhere(color, cx)
            }));
        }
        let v = view.clone();
        menu = menu.item(
            PopupMenuItem::element(move |_, _| {
                h_flex().w_full().justify_between().gap_4().child("Color…").children(color_keys.clone().map(Kbd::new))
            })
                .on_click(move |_, _, app| v.update(app, |this, cx| this.menu_open_color_bar(cx))),
        );
    }
    if let (Some(cell), Some(custom)) = (target.cell.clone(), filter_custom)
        && target.source == MenuSource::Parsed
    {
        menu = separate(menu, &mut group);
        let condition = condition_label(&cell, custom);
        let (show, hide) = (view.clone(), view.clone());
        menu = menu
            .item(
                PopupMenuItem::new(format!("Show only {condition}"))
                    .on_click(move |_, _, app| show.update(app, |this, cx| this.menu_filter(false, cx))),
            )
            .item(
                PopupMenuItem::new(format!("Hide {condition}"))
                    .on_click(move |_, _, app| hide.update(app, |this, cx| this.menu_filter(true, cx))),
            );
        if target.rule {
            let v = view.clone();
            menu = menu.item(
                PopupMenuItem::new(format!("Highlight rows where {condition}…"))
                    .on_click(move |_, _, app| v.update(app, |this, cx| this.menu_highlight_rows(cx))),
            );
        }
    }
    menu = under_pointer_items(menu, &target, view, &mut group);
    menu
}

/// Change or remove the Marker and the Text Highlight under the pointer.
fn under_pointer_items(mut menu: PopupMenu, target: &SelectionTarget, view: &Entity<AppView>, group: &mut bool) -> PopupMenu {
    if target.marker.is_some() {
        menu = separate(menu, group);
        let (change, remove) = (view.clone(), view.clone());
        menu = menu
            .item(
                PopupMenuItem::new("Change marker color…")
                    .on_click(move |_, _, app| change.update(app, |this, cx| this.menu_recolor_marker(cx))),
            )
            .item(
                PopupMenuItem::new("Remove marker")
                    .on_click(move |_, _, app| remove.update(app, |this, cx| this.menu_remove_marker(cx))),
            );
    }
    if target.highlight.is_some() {
        menu = separate(menu, group);
        let (change, remove) = (view.clone(), view.clone());
        menu = menu
            .item(
                PopupMenuItem::new("Change highlight color…")
                    .on_click(move |_, _, app| change.update(app, |this, cx| this.menu_recolor_highlight(cx))),
            )
            .item(
                PopupMenuItem::new("Remove highlight")
                    .on_click(move |_, _, app| remove.update(app, |this, cx| this.menu_remove_highlight(cx))),
            );
    }
    menu
}

/// A separator before a group, when a group came before it.
fn separate(menu: PopupMenu, group: &mut bool) -> PopupMenu {
    let menu = if *group { menu.separator() } else { menu };
    *group = true;
    menu
}

/// The condition the filter items name: `level = error`, `message contains “time”`.
pub fn condition_label(cell: &CellValue, custom: bool) -> String {
    let column = crate::columns::default_name(&cell.column).to_lowercase();
    let value = short_text(&cell.value, LABEL_CHARS);
    if custom { format!("{column} contains “{value}”") } else { format!("{column} = {value}") }
}

/// A row of the menu: a label and the colors to pick. Clicking a color runs
/// `apply` with it; the menu then closes.
fn swatch_row(
    id: &'static str,
    label: &'static str,
    colors: &[HighlightColor],
    tone: Tone,
    view: &Entity<AppView>,
    apply: fn(&mut AppView, HighlightColor, &mut gpui::Context<AppView>),
) -> PopupMenuItem {
    let colors = colors.to_vec();
    let view = view.clone();
    PopupMenuItem::element(move |_, _| {
        let mut swatches = h_flex().gap_1();
        for (i, color) in colors.iter().enumerate() {
            let (v, pick) = (view.clone(), color.clone());
            swatches = swatches.child(
                div()
                    .id(gpui::ElementId::Name(format!("{id}-{i}").into()))
                    .size(px(14.0))
                    .rounded(px(3.0))
                    .border_1()
                    .border_color(gpui::black().opacity(0.25))
                    .bg(gpui::rgb(color.rgb(tone)))
                    .cursor_pointer()
                    // The row then closes the menu.
                    .on_click(move |_, _, app| v.update(app, |this, cx| apply(this, pick.clone(), cx))),
            );
        }
        h_flex().w_full().justify_between().gap_4().child(label).child(swatches)
    })
}
