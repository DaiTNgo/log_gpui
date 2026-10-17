//! Right context sidebar (Files / Investigations / Saved Views / Markers) for the
//! active project and focused log file, built from gpui-kit components.
//! Independent from the Inspector.

use crate::app::AppView;
use crate::workspace::state::{ContextSidebarState, ContextTab, WorkspaceState, CONTEXT_SIDEBAR_WIDTH};
use crate::workspace::working_set::TabTarget;
use gpui::{px, Entity, IntoElement, ParentElement, Styled};
use gpui_component::button::{Button, ButtonVariants as _};
use crate::app::rename::RenameState;
use crate::ui::file_menu::{push_file, FileRef};
use crate::ui::menu_list::MenuList;
use crate::ui::sidebar::file_rename_input;
use crate::ui::sidebar_group::ActionGroup;
use gpui_component::sidebar::{Sidebar, SidebarMenuItem};
use gpui_component::tab::{Tab, TabBar};
use gpui_component::tag::Tag;
use gpui_component::{Side, Sizable as _};
use gpui_kit::assets::IconName;

pub struct ContextSidebar;

impl ContextSidebar {
    pub fn render(
        state: &ContextSidebarState,
        workspace: &WorkspaceState,
        all_entries: &[crate::model::LogEntry],
        visible_indices: &[usize],
        renaming: Option<&RenameState>,
        cx: &mut gpui::Context<AppView>,
    ) -> gpui::AnyElement {
        let view = cx.entity();
        let (title, menu) = match state.active_tab {
            ContextTab::Files => ("Files", Self::files_menu(workspace, &view, renaming)),
            ContextTab::Investigations => {
                ("Investigations", Self::investigations_menu(workspace, &view))
            }
            ContextTab::SavedViews => ("Saved Views", Self::saved_views_menu(workspace, &view)),
            ContextTab::Markers => ("Markers", Self::markers_menu(all_entries, visible_indices, &view)),
        };

        let tabs = TabBar::new("context-tabs")
            .segmented()
            .selected_index(ContextTab::ALL.iter().position(|t| *t == state.active_tab).unwrap_or(0))
            .on_click(cx.listener(|this, ix: &usize, _window, cx| {
                if let Some(tab) = ContextTab::ALL.get(*ix) {
                    this.show_context_tab(*tab, cx);
                }
            }))
            .children(ContextTab::ALL.iter().map(|t| Tab::new().label(t.as_str())));

        let mut group = ActionGroup::new(title);
        let add_view = view.clone();
        group = match state.active_tab {
            ContextTab::Files => group.action(move |_, _| {
                let v = add_view.clone();
                Self::add_button("ctx-add-file", move |this, cx| this.open_file_dialog(cx), v)
            }),
            ContextTab::Investigations => group.action(move |_, _| {
                let v = add_view.clone();
                Self::add_button("ctx-new-case", move |this, cx| this.create_investigation_case(cx), v)
            }),
            ContextTab::SavedViews => group.action(move |_, _| {
                let v = add_view.clone();
                Self::add_button("ctx-save-view", move |this, cx| this.save_current_view(cx), v)
            }),
            ContextTab::Markers => group,
        };

        Sidebar::<ActionGroup<MenuList>>::new("context-sidebar")
            .side(Side::Right)
            .collapsible(false)
            .w(px(CONTEXT_SIDEBAR_WIDTH))
            .header(tabs)
            .child(group.child(menu))
            .into_any_element()
    }

    fn add_button(
        id: &'static str,
        f: impl Fn(&mut AppView, &mut gpui::Context<AppView>) + 'static,
        view: Entity<AppView>,
    ) -> Button {
        Button::new(id)
            .ghost()
            .xsmall()
            .icon(IconName::Plus)
            .on_click(move |_, _, app| view.update(app, |this, cx| f(this, cx)))
    }

    /// Files of the active project with open/active state and an "Add Log File..." entry.
    fn files_menu(
        workspace: &WorkspaceState,
        view: &Entity<AppView>,
        renaming: Option<&RenameState>,
    ) -> MenuList {
        let mut menu = MenuList::new();
        if let Some(project) = workspace.active_project() {
            let active_file_id = project.working_set.active_tab().and_then(|t| match &t.target {
                TabTarget::LogFile { file_id } => Some(file_id.clone()),
                _ => None,
            });
            for file in &project.files {
                menu = push_file(
                    menu,
                    FileRef {
                        view: view.clone(),
                        project_id: project.id.clone(),
                        file_id: file.file_id.clone(),
                        display_name: file.display_name.clone(),
                        rename_input: file_rename_input(renaming, &project.id, &file.file_id),
                    },
                    active_file_id.as_deref() == Some(file.file_id.as_str()),
                    Some(IconName::FileText),
                );
            }
        }
        menu
    }

    fn investigations_menu(workspace: &WorkspaceState, view: &Entity<AppView>) -> MenuList {
        let mut menu = MenuList::new();
        if let Some(project) = workspace.active_project() {
            for case in &project.investigation_cases {
                let case_view = view.clone();
                let case_id = case.id.clone();
                let anchors = case.anchors.len();
                menu = menu.child(
                    SidebarMenuItem::new(format!("{} · {}", case.id, case.title))
                        .icon(IconName::BookOpen)
                        .on_click(move |_, _, app| {
                            let id = case_id.clone();
                            case_view.update(app, |this, cx| this.open_investigation_case(&id, cx));
                        })
                        .suffix(move |_, _| Tag::secondary().child(format!("{}", anchors))),
                );
            }
        }
        menu
    }

    fn saved_views_menu(workspace: &WorkspaceState, view: &Entity<AppView>) -> MenuList {
        let mut menu = MenuList::new();
        if let Some(project) = workspace.active_project() {
            for saved in &project.saved_views {
                let apply_view = view.clone();
                let view_id = saved.id.clone();
                let filters = saved.filter.conditions.len();
                menu = menu.child(
                    SidebarMenuItem::new(saved.name.clone())
                        .on_click(move |_, _, app| {
                            let id = view_id.clone();
                            apply_view.update(app, |this, cx| this.apply_saved_view(&id, cx));
                        })
                        .suffix(move |_, _| Tag::secondary().child(format!("{}", filters))),
                );
            }
        }
        menu
    }

    fn markers_menu(
        all_entries: &[crate::model::LogEntry],
        visible_indices: &[usize],
        view: &Entity<AppView>,
    ) -> MenuList {
        let mut menu = MenuList::new();
        for (vis_pos, &entry_ix) in visible_indices.iter().enumerate() {
            let Some(entry) = all_entries.get(entry_ix) else { continue };
            let Some(marker) = entry.effective_marker() else { continue };
            let jump_view = view.clone();
            menu = menu.child(
                SidebarMenuItem::new(format!("{} #{} {}", marker.symbol(), entry.index, entry.payload))
                    .on_click(move |_, _, app| {
                        jump_view.update(app, |this, cx| this.jump_to_row(vis_pos, cx));
                    }),
            );
        }
        menu
    }
}
