//! Project row for the left sidebar: a `SidebarItem` whose collapse/expand caret sits
//! right next to the project name (revealed on hover), with hover-revealed actions on the
//! right and the project's files rendered below as a gpui-kit `SidebarMenu`.
//!
//! gpui-kit's `SidebarMenuItem` always puts its caret at the far right, so this implements
//! the public `SidebarItem` trait itself and mirrors the menu item's styling tokens.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    div, App, ClickEvent, ElementId, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window,
};
use crate::ui::menu_list::MenuList;
use gpui_component::sidebar::SidebarItem;
use gpui_component::{h_flex, v_flex, ActiveTheme as _, Collapsible, Icon};
use gpui_kit::assets::IconName;
use std::rc::Rc;

type ActionBuilder = Rc<dyn Fn(&mut Window, &mut App) -> gpui::AnyElement>;
type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

#[derive(Clone)]
pub struct ProjectNode {
    name: SharedString,
    default_open: bool,
    collapsed: bool,
    on_click: ClickHandler,
    actions: Vec<ActionBuilder>,
    files: MenuList,
    /// Replaces the name (and hides the actions) while renaming inline.
    rename: Option<ActionBuilder>,
}

impl gpui::prelude::FluentBuilder for ProjectNode {}

impl ProjectNode {
    pub fn new(name: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            default_open: false,
            collapsed: false,
            on_click: Rc::new(|_, _, _| {}),
            actions: Vec::new(),
            files: MenuList::new(),
            rename: None,
        }
    }

    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
        self
    }

    /// Called when the row is clicked (the row also toggles itself).
    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Rc::new(f);
        self
    }

    /// Hover-revealed action on the right of the row.
    pub fn action<B: IntoElement>(
        mut self,
        builder: impl Fn(&mut Window, &mut App) -> B + 'static,
    ) -> Self {
        self.actions
            .push(Rc::new(move |w, cx| builder(w, cx).into_any_element()));
        self
    }

    /// Show `builder`'s element (an input) in place of the project name.
    pub fn renaming<B: IntoElement>(
        mut self,
        builder: impl Fn(&mut Window, &mut App) -> B + 'static,
    ) -> Self {
        self.rename = Some(Rc::new(move |w, cx| builder(w, cx).into_any_element()));
        self
    }

    pub fn files(mut self, files: MenuList) -> Self {
        self.files = files;
        self
    }
}

impl Collapsible for ProjectNode {
    fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }
}

impl SidebarItem for ProjectNode {
    fn render(self, id: impl Into<ElementId>, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = id.into();
        let open = window.use_keyed_state(
            SharedString::from(format!("{}-project-open", id)),
            cx,
            |_, _| self.default_open,
        );
        let is_open = *open.read(cx);
        let group: SharedString = format!("{}-project-row", id).into();

        let toggle = open.clone();
        let on_click = self.on_click.clone();
        let theme = cx.theme();
        let (accent, accent_fg, radius) =
            (theme.tokens.sidebar_accent, theme.sidebar_accent_foreground, theme.radius);

        let normal_row = h_flex()
            .id(SharedString::from(format!("{}-row", id)))
            .group(group.clone())
            .h(gpui::px(crate::ui::menu_list::ROW_HEIGHT))
            .flex_shrink_0()
            .p_2()
            .gap_2()
            .justify_between()
            .rounded(radius)
            .text_sm()
            .cursor_pointer()
            .hover(move |s| s.bg(accent.opacity(0.8)).text_color(accent_fg))
            .on_click(move |ev, window, cx| {
                on_click(ev, window, cx);
                toggle.update(cx, |v, cx| {
                    *v = !*v;
                    cx.notify();
                });
            })
            .child(
                h_flex()
                    .flex_1()
                    .gap_2()
                    .overflow_x_hidden()
                    .child(Icon::new(if is_open { IconName::FolderOpen } else { IconName::Folder }))
                    .child(self.name.clone())
                    // Collapse/expand indicator next to the title, shown on hover only.
                    .child(
                        div()
                            .invisible()
                            .group_hover(group.clone(), |s| s.visible())
                            .child(
                                Icon::new(if is_open {
                                    IconName::ChevronDown
                                } else {
                                    IconName::ChevronRight
                                })
                                .size_3(),
                            ),
                    ),
            )
            .child(
                h_flex()
                    .gap_0p5()
                    .invisible()
                    .group_hover(group, |s| s.visible())
                    .children(self.actions.iter().enumerate().map(|(ix, a)| {
                        // Action clicks must not toggle/select the row. Each wrapper needs its own
                        // id: sharing one shares the click state, so only one of them would swallow
                        // its click and the row would toggle.
                        div()
                            .id(SharedString::from(format!("{}-action-{ix}", id)))
                            .on_click(|_, _, cx| cx.stop_propagation())
                            .child(a(window, cx))
                    })),
            );

        // Inline rename keeps the exact row metrics (height, padding, icon) so nothing shifts.
        let row = match self.rename.clone() {
            // No row padding: the icon keeps its x position (px_2) and the input fills the rest.
            Some(rename) => h_flex()
                .h(gpui::px(crate::ui::menu_list::ROW_HEIGHT))
                .flex_shrink_0()
                .items_center()
                .rounded(radius)
                .text_sm()
                .child(
                    div()
                        .px_2()
                        .child(Icon::new(if is_open { IconName::FolderOpen } else { IconName::Folder })),
                )
                .child(div().flex_1().child(rename(window, cx)))
                .into_any_element(),
            None => normal_row.into_any_element(),
        };

        v_flex()
            .gap_1()
            .child(row)
            .when(is_open, |this| {
                this.child(
                    div().pl_4().child(
                        self.files
                            .collapsed(self.collapsed)
                            .render(format!("{}-files", id), window, cx),
                    ),
                )
            })
    }
}
