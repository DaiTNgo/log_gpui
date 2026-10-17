//! Log file row shared by both sidebars: click opens, double-click renames inline, and the
//! "..." button / right-click menu offer Rename and Delete.
//!
//! Delete only removes the file from the project; the file on disk is untouched.

use crate::app::AppView;
use crate::app::rename::RenameTarget;
use crate::ui::menu_list::MenuList;
use gpui::prelude::FluentBuilder as _;
use gpui::{App, Entity, InteractiveElement, IntoElement, ParentElement, Styled, Window, div, px};
use gpui_component::Sizable as _;
use gpui_component::input::{Input, InputState};
use gpui_component::menu::{PopupMenu, PopupMenuItem};
use gpui_component::sidebar::SidebarMenuItem;
use gpui_component::{Icon, h_flex};
use gpui_kit::assets::IconName;

pub use crate::ui::menu_list::ROW_HEIGHT;

/// Identity of the file a row acts on.
#[derive(Clone)]
pub struct FileRef {
    pub view: Entity<AppView>,
    pub project_id: String,
    pub file_id: String,
    pub display_name: String,
    /// Set while this file is being renamed inline.
    pub rename_input: Option<Entity<InputState>>,
}

/// Start the inline rename. Deferred so a closing popup menu cannot steal the new focus.
pub fn start_rename(file: &FileRef, window: &mut Window, app: &mut App) {
    let file = file.clone();
    window.defer(app, move |window, app| {
        let target = RenameTarget::File {
            project_id: file.project_id.clone(),
            file_id: file.file_id.clone(),
        };
        let name = file.display_name.clone();
        file.view
            .update(app, |this, cx| this.begin_rename(target, name, window, cx));
    });
}

/// Inline rename input without border or focus ring: Enter / blur commit (handled by
/// `AppView`), Escape cancels.
pub fn rename_field(view: Entity<AppView>, input: Entity<InputState>) -> impl IntoElement {
    // Hard 28px box (min = max) so the library's own input metrics cannot change the row height.
    div()
        .w_full()
        .h(px(ROW_HEIGHT))
        .min_h(px(ROW_HEIGHT))
        .max_h(px(ROW_HEIGHT))
        .flex()
        .items_center()
        .overflow_hidden()
        .on_key_down(move |event, _, app| {
            if event.keystroke.key == "escape" {
                view.update(app, |this, cx| this.cancel_rename(cx));
            }
        })
        // `small`: text_sm like the menu rows and 8px side padding, so the typed text lands where
        // the label was. Vertical padding is zeroed; the box above fixes the height.
        .child(
            Input::new(&input)
                .small()
                .bordered(false)
                .focus_bordered(false)
                .h(px(ROW_HEIGHT)),
        )
}

/// A whole row for the renaming file: same height as a menu item, optional leading icon at the
/// menu item's x position, and the input filling the rest of the row.
fn rename_row(
    view: Entity<AppView>,
    input: Entity<InputState>,
    icon: Option<IconName>,
) -> gpui::AnyElement {
    h_flex()
        .h(px(ROW_HEIGHT))
        .w_full()
        .flex_shrink_0()
        .items_center()
        .when_some(icon, |this, icon| {
            this.child(div().px_2().child(Icon::new(icon)))
        })
        .child(div().flex_1().child(rename_field(view, input)))
        .into_any_element()
}

/// Rename + Delete entries, shared by the "..." dropdown and the context menu.
fn file_menu_items(menu: PopupMenu, file: &FileRef) -> PopupMenu {
    let rename = file.clone();
    let delete = file.clone();
    menu.item(
        PopupMenuItem::new("Rename").on_click(move |_, window, app| {
            start_rename(&rename, window, app);
        }),
    )
    .item(PopupMenuItem::new("Delete").on_click(move |_, _, app| {
        let (project, file_id) = (delete.project_id.clone(), delete.file_id.clone());
        delete.view.update(app, |this, cx| {
            this.remove_file_from_project(&project, &file_id, cx)
        });
    }))
}

/// Append a log file row to `list`. `icon` is the optional leading icon. Rename / Delete live in
/// the right-click menu; double-click renames. While the file is renamed, a custom full-width row with the
/// input replaces the menu item.
pub fn push_file(list: MenuList, file: FileRef, active: bool, icon: Option<IconName>) -> MenuList {
    if let Some(input) = file.rename_input.clone() {
        let view = file.view.clone();
        return list.custom(move |_, _| rename_row(view.clone(), input.clone(), icon.clone()));
    }

    let open = file.clone();
    let context = file.clone();
    let mut item = SidebarMenuItem::new(file.display_name.clone())
        .active(active)
        .on_click(move |ev, window, app| {
            let id = open.file_id.clone();
            open.view
                .update(app, |this, cx| this.open_file_from_sidebar(&id, cx));
            // Double-click renames in place.
            if ev.click_count() >= 2 {
                start_rename(&open, window, app);
            }
        })
        .context_menu(move |menu, _, _| file_menu_items(menu, &context));
    if let Some(icon) = icon {
        item = item.icon(icon);
    }
    list.child(item)
}
