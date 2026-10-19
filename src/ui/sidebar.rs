//! Left sidebar: project tree (projects -> Log Spaces -> files) built from gpui-kit `Sidebar`
//! components.

use crate::app::rename::{RenameState, RenameTarget};
use crate::ui::space_menu::{FileRef, SpaceRef, push_file, push_space, rename_field};
use crate::ui::menu_list::MenuList;
use crate::ui::project_node::ProjectNode;
use crate::ui::sidebar_group::ActionGroup;
use crate::workspace::state::{LeftSidebarState, WorkspaceState};
use crate::workspace::working_set::TabTarget;
use gpui::prelude::FluentBuilder as _;
use gpui::{
    AppContext, ElementId, InteractiveElement, IntoElement, MouseButton, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_component::sidebar::Sidebar;
use gpui_component::{Side, Sizable as _};
use gpui_kit::assets::IconName;

#[derive(Clone)]
pub struct ResizeDrawer;

impl Render for ResizeDrawer {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
    }
}

/// The inline-rename input for this file, if it is the one being renamed.
pub(crate) fn file_rename_input(
    renaming: Option<&RenameState>,
    project_id: &str,
    space_id: &str,
    file_id: &str,
) -> Option<gpui::Entity<gpui_component::input::InputState>> {
    renaming.and_then(|r| match &r.target {
        RenameTarget::File { project_id: p, space_id: s, file_id: f }
            if p == project_id && s == space_id && f == file_id =>
        {
            Some(r.input.clone())
        }
        _ => None,
    })
}

/// The inline-rename input for this Log Space, if it is the one being renamed.
pub(crate) fn space_rename_input(
    renaming: Option<&RenameState>,
    project_id: &str,
    space_id: &str,
) -> Option<gpui::Entity<gpui_component::input::InputState>> {
    renaming.and_then(|r| match &r.target {
        RenameTarget::LogSpace { project_id: p, space_id: s } if p == project_id && s == space_id => {
            Some(r.input.clone())
        }
        _ => None,
    })
}

fn project_rename_input(
    renaming: Option<&RenameState>,
    project_id: &str,
) -> Option<gpui::Entity<gpui_component::input::InputState>> {
    renaming.and_then(|r| match &r.target {
        RenameTarget::Project { id } if id == project_id => Some(r.input.clone()),
        _ => None,
    })
}

pub struct SidebarView;

impl SidebarView {
    pub fn render(
        sidebar: &LeftSidebarState,
        workspace: &WorkspaceState,
        renaming: Option<&RenameState>,
        expanded_spaces: &std::collections::HashSet<String>,
        no_timestamp_files: &std::collections::HashSet<String>,
        cx: &mut gpui::Context<crate::app::AppView>,
    ) -> gpui::AnyElement {
        if !sidebar.visible {
            return div().into_any_element();
        }
        let view = cx.entity();
        let active_project_id = workspace.active_project_id.as_deref();

        let mut nodes: Vec<ProjectNode> = Vec::new();
        for project in &workspace.projects {
            let is_active = active_project_id == Some(project.id.as_str());
            let active_target = if is_active {
                project.working_set.active_tab().map(|t| t.target.clone())
            } else {
                None
            };

            let mut files = MenuList::new();
            for space in &project.log_spaces {
                let space_active = matches!(
                    &active_target,
                    Some(TabTarget::LogSpace { space_id }) if *space_id == space.id
                );
                let expanded = expanded_spaces.contains(&space.id);
                files = push_space(
                    files,
                    SpaceRef {
                        view: view.clone(),
                        project_id: project.id.clone(),
                        space_id: space.id.clone(),
                        name: space.name.clone(),
                        kind: space.kind,
                        order: space.file_order,
                        expanded,
                        active: space_active,
                        rename_input: space_rename_input(renaming, &project.id, &space.id),
                    },
                );
                if !expanded {
                    continue;
                }
                for file in space.ordered_files() {
                    let file_active = matches!(
                        &active_target,
                        Some(TabTarget::FileView { space_id, file_id })
                            if *space_id == space.id && *file_id == file.file_id
                    );
                    files = push_file(
                        files,
                        FileRef {
                            command: match file.remote.as_ref().map(|r| &r.target) {
                                Some(crate::workspace::log_space::RemoteTarget::Command { restart_on_exit, .. }) => {
                                    Some(*restart_on_exit)
                                }
                                _ => None,
                            },
                            view: view.clone(),
                            project_id: project.id.clone(),
                            space_id: space.id.clone(),
                            file_id: file.file_id.clone(),
                            display_name: file.display_name.clone(),
                            path: file.path.clone(),
                            kind: space.kind,
                            can_delete: space.files.len() > 1,
                            no_timestamps: no_timestamp_files.contains(&file.file_id),
                            rename_input: file_rename_input(
                                renaming,
                                &project.id,
                                &space.id,
                                &file.file_id,
                            ),
                        },
                        file_active,
                    );
                }
            }

            let switch_view = view.clone();
            let switch_id = project.id.clone();
            let add_view = view.clone();
            let add_id = project.id.clone();
            let more_view = view.clone();
            let more_id = project.id.clone();
            let more_name = project.name.clone();
            nodes.push(
                ProjectNode::new(project.name.clone())
                    .default_open(is_active)
                    .when_some(project_rename_input(renaming, &project.id), |node, input| {
                        let view = view.clone();
                        node.renaming(move |_, _| rename_field(view.clone(), input.clone()))
                    })
                    .on_click(move |_, _, app| {
                        let id = switch_id.clone();
                        switch_view.update(app, |this, cx| this.switch_project_to(&id, cx));
                    })
                    // "..." menu (rename / remove) and New Log Space, shown on hover.
                    .action(move |_, _| {
                        let (menu_view, menu_id, menu_name) =
                            (more_view.clone(), more_id.clone(), more_name.clone());
                        Button::new(ElementId::from(format!("more-{}", menu_id)))
                            .ghost()
                            .xsmall()
                            .icon(IconName::Ellipsis)
                            .dropdown_menu(move |menu, _, _| {
                                let (rename_view, rename_id, rename_name) =
                                    (menu_view.clone(), menu_id.clone(), menu_name.clone());
                                let (remove_view, remove_id) = (menu_view.clone(), menu_id.clone());
                                menu.item(PopupMenuItem::new("Rename Project").on_click(
                                    move |_, window, app| {
                                        let (view, id, name) =
                                            (rename_view.clone(), rename_id.clone(), rename_name.clone());
                                        // Deferred so the closing menu cannot steal the input focus.
                                        window.defer(app, move |window, app| {
                                            view.update(app, |this, cx| {
                                                this.begin_rename(
                                                    RenameTarget::Project { id },
                                                    name,
                                                    window,
                                                    cx,
                                                )
                                            });
                                        });
                                    },
                                ))
                                .item(PopupMenuItem::separator())
                                .item(PopupMenuItem::new("Remove from Sidebar").on_click(
                                    move |_, _, app| {
                                        let id = remove_id.clone();
                                        remove_view
                                            .update(app, |this, cx| this.remove_project_at(&id, cx));
                                    },
                                ))
                            })
                    })
                    .action(move |_, _| {
                        let (view, project_id) = (add_view.clone(), add_id.clone());
                        Button::new(ElementId::from(format!("add-space-{}", project_id)))
                            .ghost()
                            .xsmall()
                            .icon(IconName::Plus)
                            .on_click(move |_, _, app| {
                                let project_id = project_id.clone();
                                view.update(app, |this, cx| {
                                    this.switch_project_to(&project_id, cx);
                                    this.open_new_space_dialog(cx);
                                });
                            })
                    })
                    .files(files),
            );
        }

        let new_project_view = view.clone();
        let sidebar_el = Sidebar::<ActionGroup<ProjectNode>>::new("projects-sidebar")
            .side(Side::Left)
            .collapsible(false)
            .w(px(sidebar.width))
            .child(
                ActionGroup::new("Projects")
                    .action(move |_, _| {
                        let view = new_project_view.clone();
                        Button::new("new-project")
                            .icon(IconName::FolderPlus)
                            .ghost()
                            .xsmall()
                            .on_click(move |_, _, app| {
                                view.update(app, |this, cx| this.create_new_project(cx))
                            })
                    })
                    .children(nodes),
            );

        let view_drag_move = cx.entity();
        let is_resizing = sidebar.is_resizing;
        let resize_handle = div()
            .id(ElementId::from("drawer_resize_handle"))
            .occlude()
            .cursor_col_resize()
            .absolute()
            .top_0()
            .bottom_0()
            .right_0()
            .w(px(5.0))
            .h_full()
            .when(is_resizing, |s| s.opacity(0.6))
            .on_drag(ResizeDrawer, move |drag, _point, _window, cx| {
                cx.stop_propagation();
                cx.new(|_| drag.clone())
            })
            .on_drag_move(move |e: &gpui::DragMoveEvent<ResizeDrawer>, _window, cx| {
                view_drag_move.update(cx, |this, cx| {
                    this.update_drawer_resize(e.event.position.x, cx);
                });
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &gpui::MouseDownEvent, _window, cx| {
                    if event.click_count >= 2 {
                        this.reset_drawer_width(cx);
                    } else {
                        this.start_resizing_drawer(cx);
                    }
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.finish_resizing_drawer(cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.finish_resizing_drawer(cx)),
            );

        div()
            .relative()
            .h_full()
            .flex_shrink_0()
            .child(sidebar_el)
            .child(resize_handle)
            .into_any_element()
    }
}
