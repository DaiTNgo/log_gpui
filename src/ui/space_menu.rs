//! Log Space and file rows of the left sidebar: a Log Space row with a chevron,
//! a kind icon and a right-click menu, and its files listed below.
//!
//! Imported files are never renamed or deleted on disk; "Remove from Log
//! Space" only drops them from the list. Scratch files can be renamed, saved
//! as a copy and deleted (not the last one).

use crate::app::AppView;
use crate::app::remote_picker::EditMember;
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

/// A menu item for an action that cannot be undone, in the danger color.
pub fn danger_item(label: &'static str) -> PopupMenuItem {
    PopupMenuItem::element(move |_, cx| div().text_color(cx.theme().danger).child(label))
}

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

/// A Live Glob group row of the sidebar.
#[derive(Clone)]
pub struct GlobRef {
    pub view: Entity<AppView>,
    pub space_id: String,
    pub glob_id: String,
    /// `<server>:<path>`.
    pub label: String,
    pub stopped: bool,
    pub collapsed: bool,
    pub waiting_for_folder: bool,
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
    /// The file is a Live Glob's and no longer exists on its Server.
    pub gone: bool,
    /// The file is a Remote Copy, read whole from a Server.
    pub is_copy: bool,
    /// Tooltip of a Remote Copy: when it was read and what.
    pub copy_tip: Option<String>,
    /// Shown under a Live Glob row.
    pub in_glob: bool,
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
            let clear_all = space.clone();
            let full = space.clone();
            menu.item(PopupMenuItem::new("Add remote files…").on_click(move |_, _, app| {
                let id = add.space_id.clone();
                add.view.update(app, |this, cx| this.add_remote_files_dialog(&id, cx));
            }))
            .item(PopupMenuItem::new("Add remote command…").on_click(move |_, _, app| {
                let id = add_command.space_id.clone();
                add_command.view.update(app, |this, cx| this.add_remote_command_dialog(&id, cx));
            }))
            .item(PopupMenuItem::new("Open in full…").on_click(move |_, _, app| {
                let id = full.space_id.clone();
                full.view.update(app, |this, cx| this.open_in_full_space(&id, cx));
            }))
            .item(PopupMenuItem::new("Download all").on_click(move |_, _, app| {
                let id = download.space_id.clone();
                download.view.update(app, |this, cx| this.download_space(&id, cx));
            }))
            .item(PopupMenuItem::new("Clear all files").on_click(move |_, _, app| {
                let id = clear_all.space_id.clone();
                clear_all.view.update(app, |this, cx| this.request_clear_all_files(&id, cx));
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
    .item(danger_item("Delete").on_click(move |_, _, app| {
        let id = delete.space_id.clone();
        delete.view.update(app, |this, cx| this.request_delete_log_space(&id, cx));
    }))
}

/// The right-click menu of a file.
fn file_menu(menu: PopupMenu, file: &FileRef, cx: &mut App) -> PopupMenu {
    match file.kind {
        LogSpaceKind::Imported => {
            let remove = file.clone();
            let reveal = file.clone();
            let again = file.clone();
            let menu = if file.is_copy {
                menu.item(PopupMenuItem::new("Read again").on_click(move |_, _, app| {
                    let id = again.file_id.clone();
                    again.view.update(app, |this, cx| this.read_again(&id, cx));
                }))
            } else {
                menu
            };
            let menu = local_file_live_items(menu, file, cx);
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
            let edit = file.clone();
            let restart_on_exit = file.command.unwrap_or(false);
            menu.item(PopupMenuItem::new("Edit…").on_click(move |_, _, app| {
                let (space, id) = (edit.space_id.clone(), edit.file_id.clone());
                edit.view.update(app, |this, cx| this.open_remote_edit(&space, EditMember::Command(id), cx));
            }))
            .item(PopupMenuItem::new("Restart").on_click(move |_, _, app| {
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
            let full = file.clone();
            let edit = file.clone();
            let menu = remote_file_live_items(menu, file, cx);
            // A file a Live Glob brought is edited through the Live Glob.
            let menu = if file.in_glob {
                menu
            } else {
                menu.item(PopupMenuItem::new("Edit…").on_click(move |_, _, app| {
                    let (space, id) = (edit.space_id.clone(), edit.file_id.clone());
                    edit.view.update(app, |this, cx| this.open_remote_edit(&space, EditMember::File(id), cx));
                }))
            };
            menu.item(PopupMenuItem::new("Open in full…").on_click(move |_, _, app| {
                let (space, id) = (full.space_id.clone(), full.file_id.clone());
                full.view.update(app, |this, cx| this.open_in_full_file(&space, &id, cx));
            }))
            .item(PopupMenuItem::new("Download").on_click(move |_, _, app| {
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
                danger_item("Delete")
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

/// Load More and Back to Live Window of a local file followed live, with what
/// the file holds on top. Nothing for a file that is not followed.
fn local_file_live_items(menu: PopupMenu, file: &FileRef, cx: &App) -> PopupMenu {
    let (info, step) = {
        let view = file.view.read(cx);
        (view.hold_info(&file.file_id).filter(|i| i.local), view.live.load_more_step)
    };
    let Some(info) = info else { return menu };
    let (more, back, custom) = (file.clone(), file.clone(), file.clone());
    let off = info.load_more_off.is_some() || info.loading;
    let extended = info.extended;
    menu.item(PopupMenuItem::new(info.tooltip()).disabled(true))
        .item(
            PopupMenuItem::new(format!("Load more ({})", info.amount_label(step)))
                .disabled(off)
                .on_click(move |_, _, app| {
                    let id = more.file_id.clone();
                    more.view.update(app, |this, cx| this.load_more(&id, None, cx));
                }),
        )
        .item(PopupMenuItem::new("Load more (custom number)…").disabled(off).on_click(move |_, window, app| {
            let id = custom.file_id.clone();
            custom.view.update(app, |this, cx| this.open_custom_load(&id, window, cx));
        }))
        .item(PopupMenuItem::new("Back to Live Window").disabled(!extended).on_click(move |_, _, app| {
            let id = back.file_id.clone();
            back.view.update(app, |this, cx| this.back_to_live_window(&id, cx));
        }))
        .item(PopupMenuItem::separator())
}

/// Clear, Load More and Back to Live Window of a Remote File, with what the
/// file holds on top.
fn remote_file_live_items(menu: PopupMenu, file: &FileRef, cx: &App) -> PopupMenu {
    let (info, step) = {
        let view = file.view.read(cx);
        (view.hold_info(&file.file_id), view.live.load_more_step)
    };
    let Some(info) = info else { return menu };
    let clear = file.clone();
    if info.command {
        return menu
            .item(PopupMenuItem::new(info.tooltip()).disabled(true))
            .item(PopupMenuItem::new("Clear").on_click(move |_, _, app| {
                let id = clear.file_id.clone();
                clear.view.update(app, |this, cx| this.request_clear_file(&id, cx));
            }))
            .item(PopupMenuItem::separator());
    }
    let more = file.clone();
    let back = file.clone();
    let extended = info.extended;
    // At the Full Read Limit the button opens the whole file instead.
    let load_off = (info.load_more_off.is_some()
        && info.load_more_off != Some(crate::app::live_more::AT_FULL_READ_LIMIT))
        || info.loading;
    let loading = info.loading;
    let cancel = file.clone();
    let custom = file.clone();
    let menu = menu
        .item(PopupMenuItem::new(info.tooltip()).disabled(true))
        .item(PopupMenuItem::new("Clear").on_click(move |_, _, app| {
            let id = clear.file_id.clone();
            clear.view.update(app, |this, cx| this.request_clear_file(&id, cx));
        }))
        .item(
            PopupMenuItem::new(format!("Load more ({})", info.amount_label(step)))
                .disabled(load_off)
                .on_click(move |_, _, app| {
                    let id = more.file_id.clone();
                    more.view.update(app, |this, cx| this.load_more(&id, None, cx));
                }),
        );
    let mut menu = menu;
    for lines in [1_000usize, 10_000, 100_000] {
        let file = file.clone();
        menu = menu.item(
            PopupMenuItem::new(format!("Load more ({} lines)", info.amount_label(lines)))
            .disabled(load_off)
            .on_click(move |_, _, app| {
                let id = file.file_id.clone();
                file.view.update(app, |this, cx| this.load_more(&id, Some(lines), cx));
            }),
        );
    }
    let menu = menu.item(PopupMenuItem::new("Load more (custom number)…").disabled(load_off).on_click(
        move |_, window, app| {
            let id = custom.file_id.clone();
            custom.view.update(app, |this, cx| this.open_custom_load(&id, window, cx));
        },
    ));
    let menu = if loading {
        menu.item(PopupMenuItem::new("Cancel loading").on_click(move |_, _, app| {
            let id = cancel.file_id.clone();
            cancel.view.update(app, |this, _| this.cancel_load_more(&id));
        }))
    } else {
        menu
    };
    menu.item(PopupMenuItem::new("Back to Live Window").disabled(!extended).on_click(
        move |_, _, app| {
            let id = back.file_id.clone();
            back.view.update(app, |this, cx| this.back_to_live_window(&id, cx));
        },
    ))
    .item(PopupMenuItem::separator())
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

/// Append a Live Glob row: a group that can be collapsed, with its files
/// listed under it.
pub fn push_glob(list: MenuList, glob: GlobRef) -> MenuList {
    list.custom(move |_, cx| glob_row(glob.clone(), cx))
}

fn glob_row(glob: GlobRef, cx: &mut App) -> gpui::AnyElement {
    let theme = cx.theme();
    let (accent, accent_fg, radius, muted) =
        (theme.tokens.sidebar_accent, theme.sidebar_accent_foreground, theme.radius, theme.muted_foreground);
    let toggle_view = glob.view.clone();
    let toggle_id = glob.glob_id.clone();
    let menu_glob = glob.clone();
    h_flex()
        .id(SharedString::from(format!("glob-row-{}", glob.glob_id)))
        .h(px(ROW_HEIGHT))
        .w_full()
        .flex_shrink_0()
        .pl_5()
        .pr_2()
        .gap_1p5()
        .items_center()
        .rounded(radius)
        .text_sm()
        .cursor_pointer()
        .hover(move |s| s.bg(accent.opacity(0.8)).text_color(accent_fg))
        .on_click(move |_, _, app| {
            let id = toggle_id.clone();
            toggle_view.update(app, |this, cx| this.toggle_glob_collapsed(&id, cx));
        })
        .child(
            Icon::new(if glob.collapsed { IconName::ChevronRight } else { IconName::ChevronDown }).size_3(),
        )
        .child(Icon::new(IconName::Globe).size_4())
        .child(div().flex_1().truncate().child(glob.label.clone()))
        .when(glob.waiting_for_folder, |this| {
            this.child(div().text_xs().text_color(muted).child("waiting for folder"))
        })
        .when(glob.stopped, |this| this.child(div().text_xs().text_color(muted).child("stopped")))
        .context_menu(move |menu, _, _| glob_menu(menu, &menu_glob))
        .into_any_element()
}

/// The right-click menu of a Live Glob.
fn glob_menu(menu: PopupMenu, glob: &GlobRef) -> PopupMenu {
    let toggle = glob.clone();
    let remove = glob.clone();
    let edit = glob.clone();
    let label = if glob.stopped { "Resume watching" } else { "Stop watching" };
    menu.item(PopupMenuItem::new("Edit…").on_click(move |_, _, app| {
        let (space, id) = (edit.space_id.clone(), edit.glob_id.clone());
        edit.view.update(app, |this, cx| this.open_remote_edit(&space, EditMember::Glob(id), cx));
    }))
    .item(PopupMenuItem::new(label).on_click(move |_, _, app| {
        let (space, id) = (toggle.space_id.clone(), toggle.glob_id.clone());
        let stopped = toggle.stopped;
        toggle.view.update(app, |this, cx| {
            if stopped {
                this.resume_watching_glob(&space, &id, cx)
            } else {
                this.stop_watching_glob(&space, &id, cx)
            }
        });
    }))
    .item(PopupMenuItem::separator())
    .item(PopupMenuItem::new("Remove Live Glob").on_click(move |_, _, app| {
        let (space, id) = (remove.space_id.clone(), remove.glob_id.clone());
        remove.view.update(app, |this, cx| this.remove_live_glob(&space, &id, cx));
    }))
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
    let hold_view = file.view.clone();
    let hold_file = file.file_id.clone();
    let remote = file.kind == LogSpaceKind::Remote;
    let copy_tip = file.copy_tip.clone();
    let item = SidebarMenuItem::new(file.display_name.clone())
        .icon(if file.is_copy { IconName::Copy } else { IconName::File })
        .when_some(copy_tip, |item, tip| {
            item.suffix(move |_, cx| {
                let tip = tip.clone();
                div()
                    .id("remote-copy-badge")
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("copy")
                    .tooltip(move |window, cx| gpui_component::tooltip::Tooltip::new(tip.clone()).build(window, cx))
            })
        })
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
        .when(remote && !no_timestamps, |item| {
            item.suffix(move |_, cx| {
                let info = hold_view.read(cx).hold_info(&hold_file);
                let muted = cx.theme().muted_foreground;
                let text = info.as_ref().map(|i| i.tooltip()).unwrap_or_default();
                let short = info
                    .as_ref()
                    .map(|i| format!("{} MB{}", i.approx_mb(), if i.extended { " · Ext" } else { "" }))
                    .unwrap_or_default();
                div()
                    .id(SharedString::from(format!("hold-{hold_file}")))
                    .text_xs()
                    .text_color(muted)
                    .child(short)
                    .when(!text.is_empty(), |d| {
                        d.tooltip(move |window, cx| {
                            gpui_component::tooltip::Tooltip::new(text.clone()).build(window, cx)
                        })
                    })
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
        .context_menu(move |menu, _, cx| {
            let menu = file_menu(menu, &context, cx);
            file_format_items(menu, &context, cx)
        });
    if file.in_glob { list.grouped_child(item, file.gone) } else { list.nested_child(item) }
}

/// The ElementId for rows keyed by Log Space.
#[allow(dead_code)]
pub fn space_element_id(space_id: &str) -> ElementId {
    ElementId::from(SharedString::from(format!("space-{space_id}")))
}

/// The items to choose a Log Format: for the Log Space `space_id`, or for its
/// file `file_id` when given.
pub fn format_menu(
    menu: gpui_component::menu::PopupMenu,
    view: &gpui::Entity<crate::app::AppView>,
    space_id: &str,
    file_id: Option<&str>,
    current: Option<crate::log_format::FormatChoice>,
    formats: &[(String, String)],
) -> gpui_component::menu::PopupMenu {
    use crate::log_format::FormatChoice;
    let choose = |choice: Option<FormatChoice>| {
        let (view, space, file) = (view.clone(), space_id.to_string(), file_id.map(str::to_string));
        move |_: &gpui::ClickEvent, _: &mut gpui::Window, app: &mut gpui::App| {
            let choice = choice.clone();
            view.update(app, |this, cx| match &file {
                Some(file) => this.set_file_format(file, choice, cx),
                None => this.set_space_format(&space, choice, cx),
            });
        }
    };
    let unset = if file_id.is_some() { "As the Log Space" } else { "By file name, else Auto-detect" };
    let mut menu = menu
        .item(PopupMenuItem::new(unset).checked(current.is_none()).on_click(choose(None)))
        .item(
            PopupMenuItem::new("Auto-detect")
                .checked(current == Some(FormatChoice::AutoDetect))
                .on_click(choose(Some(FormatChoice::AutoDetect))),
        );
    if !formats.is_empty() {
        menu = menu.item(PopupMenuItem::separator());
    }
    for (id, name) in formats {
        let c = FormatChoice::Format(id.clone());
        menu = menu.item(PopupMenuItem::new(name.clone()).checked(current.as_ref() == Some(&c)).on_click(choose(Some(c))));
    }
    let (new_view, edit_view) = (view.clone(), view.clone());
    let apply_to = file_id.is_none().then(|| space_id.to_string());
    menu = menu.item(PopupMenuItem::separator()).item(PopupMenuItem::new("New Log Format…").on_click(
        move |_, window, app| {
            let (view, apply_to) = (new_view.clone(), apply_to.clone());
            window.defer(app, move |window, app| {
                view.update(app, |this, cx| this.open_format_editor(None, apply_to, window, cx));
            });
        },
    ));
    if let Some(FormatChoice::Format(id)) = current {
        menu = menu.item(PopupMenuItem::new("Edit this Log Format…").on_click(move |_, window, app| {
            let (view, id) = (edit_view.clone(), id.clone());
            window.defer(app, move |window, app| {
                view.update(app, |this, cx| this.open_format_editor(Some(&id), None, window, cx));
            });
        }));
    }
    menu
}

/// The Log Format of a Source File of the active Project, chosen for the
/// file alone: a "Log Format" section of the file's menu.
fn file_format_items(menu: PopupMenu, file: &FileRef, cx: &mut App) -> PopupMenu {
    let this = file.view.read(cx);
    if this.workspace_state.active_project_id.as_deref() != Some(file.project_id.as_str()) {
        return menu;
    }
    let current = this.file_format_choice(&file.file_id);
    let formats: Vec<(String, String)> = this.log_formats().iter().map(|f| (f.id.clone(), f.name.clone())).collect();
    let menu = menu.item(PopupMenuItem::separator()).item(PopupMenuItem::label("Log Format"));
    format_menu(menu, &file.view, &file.space_id, Some(&file.file_id), current, &formats)
}
