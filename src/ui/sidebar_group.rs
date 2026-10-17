//! `SidebarGroup` with a collapse/expand toggle and header actions (e.g. `+`).
//!
//! gpui-kit's `SidebarGroup` only renders a static label, so this wrapper implements the
//! public `SidebarItem` trait itself: the label row toggles the group, and the caret and
//! action buttons are revealed while the row is hovered.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, ElementId, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, div,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::sidebar::SidebarItem;
use gpui_component::{ActiveTheme as _, Collapsible, Sizable as _, h_flex, v_flex};
use gpui_kit::assets::IconName;
use std::rc::Rc;

type ActionBuilder = Rc<dyn Fn(&mut Window, &mut App) -> gpui::AnyElement>;

#[derive(Clone)]
pub struct ActionGroup<E: SidebarItem + 'static> {
    label: SharedString,
    collapsed: bool,
    default_open: bool,
    actions: Vec<ActionBuilder>,
    children: Vec<E>,
}

impl<E: SidebarItem> ActionGroup<E> {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            collapsed: false,
            default_open: true,
            actions: Vec::new(),
            children: Vec::new(),
        }
    }

    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
        self
    }

    /// Add a header action button (shown on header hover). The builder is called on each render.
    pub fn action<B: IntoElement>(
        mut self,
        builder: impl Fn(&mut Window, &mut App) -> B + 'static,
    ) -> Self {
        self.actions
            .push(Rc::new(move |w, cx| builder(w, cx).into_any_element()));
        self
    }

    pub fn child(mut self, child: E) -> Self {
        self.children.push(child);
        self
    }

    pub fn children(mut self, children: impl IntoIterator<Item = E>) -> Self {
        self.children.extend(children);
        self
    }
}

impl<E: SidebarItem> Collapsible for ActionGroup<E> {
    fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }
}

impl<E: SidebarItem> SidebarItem for ActionGroup<E> {
    fn render(
        self,
        id: impl Into<ElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        let id = id.into();
        let open = window.use_keyed_state(
            SharedString::from(format!("{}-group-open", id)),
            cx,
            |_, _| self.default_open,
        );
        let is_open = *open.read(cx);
        let group_name: SharedString = format!("{}-group-header", id).into();

        let toggle_state = open.clone();
        let header = h_flex()
            .id(SharedString::from(format!("{}-header", id)))
            .group(group_name.clone())
            .flex_shrink_0()
            .h_8()
            .px_2()
            .justify_between()
            .rounded(cx.theme().radius)
            .text_xs()
            .text_color(cx.theme().sidebar_foreground.opacity(0.7))
            .cursor_pointer()
            .on_click(move |_, _, cx| {
                toggle_state.update(cx, |v, cx| {
                    *v = !*v;
                    cx.notify();
                });
            })
            .child(
                h_flex()
                    .gap_1()
                    .items_center()
                    .child(self.label.clone())
                    .child(
                        div()
                            .invisible()
                            .group_hover(group_name.clone(), |s| s.visible())
                            .child(
                                Button::new(SharedString::from(format!("{}-caret", id)))
                                    .ghost()
                                    .xsmall()
                                    .icon(if is_open {
                                        IconName::ChevronDown
                                    } else {
                                        IconName::ChevronRight
                                    }),
                            ),
                    ),
            )
            .child(
                h_flex()
                    .gap_0p5()
                    .invisible()
                    .group_hover(group_name, |s| s.visible())
                    .children(self.actions.iter().map(|a| {
                        // Keep an action click from toggling the group.
                        div()
                            .id("group-action")
                            .on_click(|_, _, cx| cx.stop_propagation())
                            .child(a(window, cx))
                    })),
            );

        v_flex()
            .relative()
            .when(!self.collapsed, |this| this.child(header))
            .when(is_open || self.collapsed, |this| {
                this.child(div().gap_2().flex_col().children(
                    self.children.into_iter().enumerate().map(|(ix, child)| {
                        child
                            .collapsed(self.collapsed)
                            .render(format!("{}-{}", id, ix), window, cx)
                            .into_any_element()
                    }),
                ))
            })
    }
}
