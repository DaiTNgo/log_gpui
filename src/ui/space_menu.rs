//! Log Space and file rows of the left sidebar: a Log Space row with a chevron,
//! a kind icon and a right-click menu, and its files listed below.
//!
//! Imported files are never renamed or deleted on disk; "Remove from Log
//! Space" only drops them from the list. Scratch files can be renamed, saved
//! as a copy and deleted (not the last one).

use crate::app::AppView;
use crate::app::rename::RenameTarget;
use crate::ui::menu_list::{MenuList, ROW_HEIGHT};
use crate::workspace::log_space::{FileOrder, LogSpaceKind};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, ElementId, Entity, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_component::input::{Input, InputState};
use gpui_component::menu::{ContextMenuExt as _, PopupMenu, PopupMenuItem};
use gpui_component::sidebar::SidebarMenuItem;
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, h_flex};
use gpui_kit::assets::IconName;

/// Identity and state of the Log Space a row acts on.
#[derive(Clone)]
pub struct SpaceRef {
    pub view: Entity<AppView>,
    pub project_id: String,
    pub space_id: String,
    pub name: String,
    pub kind: LogSpaceKind,
    pub order: FileOrder,
    pub expanded: bool,
    pub active: bool,
    /// Set while this Log Space is being renamed inline.
    pub rename_input: Option<Entity<InputState>>,
}

/// Identity and state of the file a row acts on.
#[derive(Clone)]
pub struct FileRef {
    /// `Some(restart_on_exit)` for a Remote Command.
    pub command: Option<bool>,
    pub view: Entity<AppView>,
    pub project_id: String,
    pub space_id: String,
    pub file_id: String,
    pub display_name: String,
    pub path: Option<String>,
    pub kind: LogSpaceKind,
    /// More than one file in the space (a scratch space keeps its last file).
    pub can_delete: bool,
    pub no_timestamps: bool,
    pub rename_input: Option<Entity<InputState>>,
}

/// Start an inline rename. Deferred so a closing popup menu cannot steal the new focus.
fn defer_rename(view: Entity<AppView>, target: RenameTarget, name: String, window: &mut Window, app: &mut App) {
    window.defer(app, move |window, app| {
        view.update(app, |this, cx| this.begin_rename(target, name, window, cx));
    });
}

/// Inline rename input without border or focus ring: Enter / blur commit (handled by
/// `AppView`), Escape cancels.
pub fn rename_field(view: Entity<AppView>, input: Entity<InputState>) -> impl IntoElement {
    // Hard box (min = max) so the library's own input metrics cannot change the row height.
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
        .child(
            Input::new(&input)
                .small()
                .bordered(false)
                .focus_bordered(false)
                .h(px(ROW_HEIGHT)),
        )
}

fn rename_row(view: Entity<AppView>, input: Entity<InputState>, icon: IconName) -> gpui::AnyElement {
    h_flex()
        .h(px(ROW_HEIGHT))
        .w_full()
        .flex_shrink_0()
        .items_center()
        .child(div().px_2().child(Icon::new(icon)))
        .child(div().flex_1().child(rename_field(view, input)))
        .into_any_element()
}

fn kind_icon(kind: LogSpaceKind) -> IconName {
    match kind {
        LogSpaceKind::Imported => IconName::Layers,
        LogSpaceKind::Scratch => IconName::FilePen,
        LogSpaceKind::Remote => IconName::Globe,
    }
}

/// The right-click / "..." menu of a Log Space.
pub fn space_menu(
    menu: PopupMenu,
    space: &SpaceRef,
    window: &mut Window,
    cx: &mut gpui::Context<PopupMenu>,
) -> PopupMenu {
    let rename = space.clone();
    let add = space.clone();
    let delete = space.clone();
    let order = space.order;
    let sort_added = space.clone();
    let sort_name = space.clone();

    let menu = menu.item(PopupMenuItem::new("Rename").on_click(move |_, window, app| {
        let target = RenameTarget::LogSpace {
            project_id: rename.project_id.clone(),
            space_id: rename.space_id.clone(),
        };
        defer_rename(rename.view.clone(), target, rename.name.clone(), window, app);
    }));
    let menu = match space.kind {
        LogSpaceKind::Imported => menu.item(PopupMenuItem::new("Add files…").on_click(
            move |_, _, app| {
                let id = add.space_id.clone();
                add.view.update(app, |this, cx| this.add_files_dialog(&id, cx));
            },
        )),
        LogSpaceKind::Scratch => menu.item(PopupMenuItem::new("New scratch file").on_click(
            move |_, _, app| {
                let id = add.space_id.clone();
                add.view.update(app, |this, cx| this.new_scratch_file_in(&id, cx));
            },
        )),
        LogSpaceKind::Remote => {
            let download = space.clone();
            let add_command = space.clone();
            menu.item(PopupMenuItem::new("Add remote files…").on_click(move |_, _, app| {
                let id = add.space_id.clone();
                add.view.update(app, |this, cx| this.add_remote_files_dialog(&id, cx));
            }))
            .item(PopupMenuItem::new("Add remote command…").on_click(move |_, _, app| {
                let id = add_command.space_id.clone();
                add_command.view.update(app, |this, cx| this.add_remote_command_dialog(&id, cx));
            }))
            .item(PopupMenuItem::new("Download all").on_click(move |_, _, app| {
                let id = download.space_id.clone();
                download.view.update(app, |this, cx| this.download_space(&id, cx));
            }))
        }
    };
    menu.submenu("Sort files by", window, cx, move |menu, _, _| {
        let (a, n) = (sort_added.clone(), sort_name.clone());
        menu.item(
            PopupMenuItem::new("Added")
                .checked(order == FileOrder::Added)
                .on_click(move |_, _, app| {
                    let id = a.space_id.clone();
                    a.view.update(app, |this, cx| this.set_file_order(&id, FileOrder::Added, cx));
                }),
        )
        .item(
            PopupMenuItem::new("Name")
                .checked(order == FileOrder::Name)
                .on_click(move |_, _, app| {
                    let id = n.space_id.clone();
                    n.view.update(app, |this, cx| this.set_file_order(&id, FileOrder::Name, cx));
                }),
        )
    })
    .item(PopupMenuItem::separator())
    .item(PopupMenuItem::new("Delete").on_click(move |_, _, app| {
        let id = delete.space_id.clone();
        delete.view.update(app, |this, cx| this.request_delete_log_space(&id, cx));
    }))
}

/// The right-click menu of a file.
fn file_menu(menu: PopupMenu, file: &FileRef) -> PopupMenu {
    match file.kind {
        LogSpaceKind::Imported => {
            let remove = file.clone();
            let reveal = file.clone();
            let menu = menu.item(PopupMenuItem::new("Remove from Log Space").on_click(
                move |_, _, app| {
                    let (space, id) = (remove.space_id.clone(), remove.file_id.clone());
                    remove
                        .view
                        .update(app, |this, cx| this.remove_file_from_space(&space, &id, cx));
                },
            ));
            menu.item(
                PopupMenuItem::new(crate::ui::platform::REVEAL_IN_FILE_MANAGER)
                    .disabled(file.path.is_none())
                    .on_click(move |_, _, app| {
                        if let Some(path) = reveal.path.clone() {
                            reveal.view.update(app, |this, _| this.reveal_in_file_manager(&path));
                        }
                    }),
            )
        }
        LogSpaceKind::Remote if file.command.is_some() => {
            // A command has no file to download.
            let restart = file.clone();
            let on_exit = file.clone();
            let remove = file.clone();
            let restart_on_exit = file.command.unwrap_or(false);
            menu.item(PopupMenuItem::new("Restart").on_click(move |_, _, app| {
                let id = restart.file_id.clone();
                restart.view.update(app, |this, cx| this.restart_remote_command(&id, cx));
            }))
            .item(PopupMenuItem::new("Restart on exit").checked(restart_on_exit).on_click(move |_, _, app| {
                let (space, id) = (on_exit.space_id.clone(), on_exit.file_id.clone());
                on_exit.view.update(app, |this, cx| this.set_restart_on_exit(&space, &id, !restart_on_exit, cx));
            }))
            .item(PopupMenuItem::separator())
            .item(PopupMenuItem::new("Remove from Log Space").on_click(move |_, _, app| {
                let (space, id) = (remove.space_id.clone(), remove.file_id.clone());
                remove.view.update(app, |this, cx| this.remove_file_from_space(&space, &id, cx));
            }))
        }
        LogSpaceKind::Remote => {
            let remove = file.clone();
            let download = file.clone();
            menu.item(PopupMenuItem::new("Download").on_click(move |_, _, app| {
                let (space, id) = (download.space_id.clone(), download.file_id.clone());
                download.view.update(app, |this, cx| this.download_file(&space, &id, cx));
            }))
            .item(PopupMenuItem::new("Remove from Log Space").on_click(move |_, _, app| {
                let (space, id) = (remove.space_id.clone(), remove.file_id.clone());
                remove.view.update(app, |this, cx| this.remove_file_from_space(&space, &id, cx));
            }))
        }
        LogSpaceKind::Scratch => {
            let rename = file.clone();
            let save_as = file.clone();
            let delete = file.clone();
            menu.item(PopupMenuItem::new("Rename").on_click(move |_, window, app| {
                let target = RenameTarget::File {
                    project_id: rename.project_id.clone(),
                    space_id: rename.space_id.clone(),
                    file_id: rename.file_id.clone(),
                };
                defer_rename(rename.view.clone(), target, rename.display_name.clone(), window, app);
            }))
            .item(PopupMenuItem::new("Save As…").on_click(move |_, _, app| {
                let (space, id) = (save_as.space_id.clone(), save_as.file_id.clone());
                save_as
                    .view
                    .update(app, |this, cx| this.save_as_dialog(&space, &id, cx));
            }))
            .item(PopupMenuItem::separator())
            .item(
                PopupMenuItem::new("Delete")
                    .disabled(!file.can_delete)
                    .on_click(move |_, _, app| {
                        let (space, id) = (delete.space_id.clone(), delete.file_id.clone());
                        delete.view.update(app, |this, cx| {
                            this.request_delete_scratch_file(&space, &id, cx)
                        });
                    }),
            )
        }
    }
}

/// Append a Log Space row to `list`.
pub fn push_space(list: MenuList, space: SpaceRef) -> MenuList {
    if let Some(input) = space.rename_input.clone() {
        let view = space.view.clone();
        let icon = kind_icon(space.kind);
        return list.custom(move |_, _| rename_row(view.clone(), input.clone(), icon.clone()));
    }
    list.custom(move |_, cx| space_row(space.clone(), cx))
}

fn space_row(space: SpaceRef, cx: &mut App) -> gpui::AnyElement {
    let theme = cx.theme();
    let (accent, accent_fg, radius) =
        (theme.tokens.sidebar_accent, theme.sidebar_accent_foreground, theme.radius);
    let open_view = space.view.clone();
    let open_id = space.space_id.clone();
    let open_project = space.project_id.clone();
    let toggle_view = space.view.clone();
    let toggle_id = space.space_id.clone();
    let menu_space = space.clone();
    let active = space.active;

    h_flex()
        .id(SharedString::from(format!("space-row-{}", space.space_id)))
        .h(px(ROW_HEIGHT))
        .w_full()
        .flex_shrink_0()
        .px_2()
        .gap_1p5()
        .items_center()
        .rounded(radius)
        .text_sm()
        .cursor_pointer()
        .when(active, |this| {
            this.font_weight(gpui::FontWeight::MEDIUM)
                .bg(accent)
                .text_color(accent_fg)
        })
        .when(!active, |this| {
            this.hover(move |s| s.bg(accent.opacity(0.8)).text_color(accent_fg))
        })
        .child(
            div()
                .id(SharedString::from(format!("space-chevron-{}", space.space_id)))
                .flex()
                .items_center()
                .on_click(move |_, _, app| {
                    app.stop_propagation();
                    let id = toggle_id.clone();
                    toggle_view.update(app, |this, cx| this.toggle_space_expand(&id, cx));
                })
                .child(
                    Icon::new(if space.expanded {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    })
                    .size_3(),
                ),
        )
        .child(Icon::new(kind_icon(space.kind)).size_4())
        .child(div().flex_1().truncate().child(space.name.clone()))
        .on_click(move |_, _, app| {
            let (project, id) = (open_project.clone(), open_id.clone());
            open_view.update(app, |this, cx| this.open_space_in_project(&project, &id, cx));
        })
        .context_menu(move |menu, window, cx| space_menu(menu, &menu_space, window, cx))
        .into_any_element()
}

/// Append a file row (indented under its Log Space) to `list`.
pub fn push_file(list: MenuList, file: FileRef, active: bool) -> MenuList {
    if let Some(input) = file.rename_input.clone() {
        let view = file.view.clone();
        return list.custom(move |_, _| {
            div()
                .pl_5()
                .child(rename_row(view.clone(), input.clone(), IconName::File))
                .into_any_element()
        });
    }

    let open = file.clone();
    let context = file.clone();
    let no_timestamps = file.no_timestamps;
    let item = SidebarMenuItem::new(file.display_name.clone())
        .icon(IconName::File)
        .when(no_timestamps, |item| {
            item.suffix(|_, cx| {
                h_flex()
                    .gap_1()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(Icon::new(IconName::Flag).size(px(12.0)))
                    .child("No timestamps")
            })
        })
        .active(active)
        .on_click(move |ev, window, app| {
            let (project, space, id) =
                (open.project_id.clone(), open.space_id.clone(), open.file_id.clone());
            open.view
                .update(app, |this, cx| this.open_file_in_project(&project, &space, &id, cx));
            // Double-click renames a scratch file in place.
            if ev.click_count() >= 2 && open.kind == LogSpaceKind::Scratch {
                let target = RenameTarget::File {
                    project_id: open.project_id.clone(),
                    space_id: open.space_id.clone(),
                    file_id: open.file_id.clone(),
                };
                defer_rename(open.view.clone(), target, open.display_name.clone(), window, app);
            }
        })
        .context_menu(move |menu, _, _| file_menu(menu, &context));
    list.nested_child(item)
}

/// The ElementId for rows keyed by Log Space.
#[allow(dead_code)]
pub fn space_element_id(space_id: &str) -> ElementId {
    ElementId::from(SharedString::from(format!("space-{space_id}")))
}
