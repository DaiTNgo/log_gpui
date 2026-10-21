//! UI: working-set tab bar built from gpui-kit `TabBar` / `Tab`.

use super::AppView;
use crate::workspace::working_set::TabTarget;
use gpui::prelude::FluentBuilder as _;
use gpui::{div, px, MouseButton, Context, InteractiveElement as _, IntoElement, ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled};
use crate::workspace::working_set::CloseScope;
use gpui_component::menu::{ContextMenuExt as _, PopupMenuItem};
use gpui_component::{h_flex, ActiveTheme as _, Icon};
use gpui_kit::assets::IconName;

/// Matches the Zed tab strip height.
const TAB_BAR_HEIGHT: f32 = 34.0;

impl AppView {
    pub(super) fn render_tab_bar(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let view = cx.entity();
        let (labels, active_idx): (Vec<(String, bool)>, Option<usize>) = match self.active_project() {
            Some(proj) => (
                proj.working_set
                    .tabs()
                    .iter()
                    .map(|tab| match &tab.target {
                        TabTarget::LogSpace { space_id } => (
                            proj.log_space(space_id).map(|s| s.name.clone()).unwrap_or_else(|| space_id.clone()),
                            false,
                        ),
                        TabTarget::FileView { space_id, file_id } => {
                            let space = proj.log_space(space_id);
                            let file = space.and_then(|s| s.file(file_id));
                            let label = match (file, space) {
                                (Some(f), Some(s)) => format!("{} · {}", f.display_name, s.name),
                                _ => file_id.clone(),
                            };
                            (label, false)
                        }
                        TabTarget::InvestigationBoard { case_id } => (case_id.clone(), true),
                    })
                    .collect(),
                proj.working_set.active_tab_index(),
            ),
            None => (Vec::new(), None),
        };

        let theme = cx.theme();
        let (border, muted, fg) = (theme.border, theme.muted_foreground, theme.foreground);
        let active_bg = crate::ui::main_view::background(cx);
        // Inactive tabs and the empty strip are darker than the active tab, as in Zed.
        let mut bar_bg = active_bg;
        bar_bg.l *= 0.55;
        let last = labels.len().saturating_sub(1);

        // Zed-style strip: flat tabs split by a hairline; the active tab takes
        // the main view's tone and drops the bottom border so it joins the
        // content below. The close button shows on hover (always when active).
        let tabs = labels.into_iter().enumerate().map(|(index, (label, board))| {
            let active = active_idx == Some(index);
            let pinned = self.active_project().is_some_and(|p| p.working_set.is_pinned(index));
            let closable = |scope| {
                self.active_project().map_or(0, |p| p.working_set.closable_count(scope, index))
            };
            let counts = [
                closable(CloseScope::Others),
                closable(CloseScope::Left),
                closable(CloseScope::Right),
                closable(CloseScope::All),
            ];
            let is_file_view = self.tab_file_path(index, false).is_some();
            let is_local = self.tab_file_is_local(index);
            let has_space = self
                .active_project()
                .and_then(|p| p.working_set.tabs().get(index))
                .is_some_and(|t| t.target.space_id().is_some());
            let menu_view = view.clone();
            let group = SharedString::from(format!("working-tab-{index}"));
            let (click_view, close_view) = (view.clone(), view.clone());
            h_flex()
                .id(("working-tab", index))
                .group(group.clone())
                .h_full()
                .flex_shrink_0()
                .px_3()
                .gap_2()
                .items_center()
                .cursor_pointer()
                .text_sm()
                .border_r_1()
                .border_color(border)
                .when(index == last, |this| this.border_r_0())
                .when(active, |this| this.bg(active_bg).text_color(fg))
                .when(!active, |this| {
                    this.bg(bar_bg).text_color(muted).hover(|s| s.text_color(fg))
                })
                .when(board, |this| this.child(Icon::new(IconName::Flag).size_3()))
                .child(label)
                .child(
                    div()
                        .id(("close-tab", index))
                        .flex()
                        .items_center()
                        .justify_center()
                        .size_4()
                        .rounded_sm()
                        .when(!active && !pinned, |this| this.invisible())
                        .group_hover(group, |s| s.visible())
                        .hover(|s| s.bg(border))
                        .child(Icon::new(if pinned { IconName::Pin } else { IconName::Close }).size_3())
                        .on_mouse_down(MouseButton::Left, |_, _, app| app.stop_propagation())
                        .on_click(move |_, _, app| {
                            close_view.update(app, |this, cx| this.close_tab_at(index, cx));
                        }),
                )
                // Switch on mouse down like Zed; re-selecting the active tab is a no-op.
                .on_mouse_down(MouseButton::Left, move |_, _, app| {
                    if !active {
                        click_view.update(app, |this, cx| this.switch_to_tab(index, cx));
                    }
                })
                .context_menu(move |menu, _, _| {
                    let [others, left, right, all] = counts;
                    let item = |label: &'static str, enabled: bool, act: Box<dyn Fn(&mut AppView, &mut Context<AppView>)>| {
                        let v = menu_view.clone();
                        PopupMenuItem::new(label)
                            .disabled(!enabled)
                            .on_click(move |_, _, app| {
                                v.update(app, |this, cx| act(this, cx));
                            })
                    };
                    let mut menu = menu
                        .item(item("Close", true, Box::new(move |this, cx| this.close_tab_at(index, cx))))
                        .item(item("Close Others", others > 0, Box::new(move |this, cx| this.close_scope_at(CloseScope::Others, index, cx))))
                        .item(PopupMenuItem::separator())
                        .item(item("Close to the Left", left > 0, Box::new(move |this, cx| this.close_scope_at(CloseScope::Left, index, cx))))
                        .item(item("Close to the Right", right > 0, Box::new(move |this, cx| this.close_scope_at(CloseScope::Right, index, cx))))
                        .item(item("Close All", all > 0, Box::new(move |this, cx| this.close_scope_at(CloseScope::All, index, cx))))
                        .item(PopupMenuItem::separator())
                        .item(item(
                            if pinned { "Unpin Tab" } else { "Pin Tab" },
                            true,
                            Box::new(move |this, cx| this.set_tab_pinned(index, !pinned, cx)),
                        ));
                    if is_file_view {
                        menu = menu
                            .item(PopupMenuItem::separator())
                            .item(item("Copy Path", true, Box::new(move |this, cx| this.copy_tab_path(index, false, cx))))
                            .item(item("Copy Relative Path", true, Box::new(move |this, cx| this.copy_tab_path(index, true, cx))));
                        if is_local {
                            menu = menu.item(item("Reveal in Finder", true, Box::new(move |this, _| this.reveal_tab_file(index))));
                        }
                    }
                    if has_space {
                        menu = menu
                            .item(PopupMenuItem::separator())
                            .item(item("Reveal in Sidebar", true, Box::new(move |this, cx| this.reveal_tab_in_sidebar(index, cx))));
                    }
                    menu
                })
        });

        h_flex()
            .id("working-set")
            .w_full()
            .h(px(TAB_BAR_HEIGHT))
            .flex_shrink_0()
            .overflow_x_scroll()
            .bg(bar_bg)
            .border_b_1()
            .border_color(border)
            .children(tabs)
            .into_any_element()
    }
}
