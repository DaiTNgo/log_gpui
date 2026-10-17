//! A `SidebarItem` list of gpui-kit `SidebarMenuItem`s that can also hold custom rows.
//!
//! `SidebarMenuItem` only takes a text label, so an inline rename input cannot live
//! inside it without inheriting the item's padding and gaps. Custom entries let the
//! renaming row render an input that fills the row exactly.

use gpui::{App, IntoElement, ParentElement, Styled, Window, div, px};
use gpui_component::sidebar::{SidebarItem, SidebarMenu, SidebarMenuItem};
use gpui_component::{Collapsible, v_flex};
use std::rc::Rc;

/// Hard height of every row in the list (gpui-kit `SidebarMenuItem` is `h_7`). Both normal and
/// custom (renaming) rows are boxed to exactly this, so editing never changes a row's height.
pub const ROW_HEIGHT: f32 = 28.0;

type CustomRow = Rc<dyn Fn(&mut Window, &mut App) -> gpui::AnyElement>;

#[derive(Clone)]
enum Entry {
    Item(SidebarMenuItem),
    Custom(CustomRow),
}

#[derive(Clone, Default)]
pub struct MenuList {
    entries: Vec<Entry>,
    collapsed: bool,
}

impl MenuList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn child(mut self, item: SidebarMenuItem) -> Self {
        self.entries.push(Entry::Item(item));
        self
    }

    pub fn custom<E: IntoElement>(
        mut self,
        builder: impl Fn(&mut Window, &mut App) -> E + 'static,
    ) -> Self {
        self.entries.push(Entry::Custom(Rc::new(move |w, cx| {
            builder(w, cx).into_any_element()
        })));
        self
    }
}

impl Collapsible for MenuList {
    fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }
}

impl SidebarItem for MenuList {
    fn render(
        self,
        id: impl Into<gpui::ElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        let id: gpui::ElementId = id.into();
        let collapsed = self.collapsed;
        // Same vertical rhythm as `SidebarMenu` (gap_2).
        v_flex()
            .gap_2()
            .children(self.entries.into_iter().enumerate().map(|(ix, entry)| {
                match entry {
                    Entry::Item(item) => div()
                        .h(px(ROW_HEIGHT))
                        .flex_shrink_0()
                        .child(SidebarMenu::new().child(item).collapsed(collapsed).render(
                            format!("{}-{}", id, ix),
                            window,
                            cx,
                        ))
                        .into_any_element(),
                    Entry::Custom(build) => div()
                        .h(px(ROW_HEIGHT))
                        .flex_shrink_0()
                        .overflow_hidden()
                        .child(build(window, cx))
                        .into_any_element(),
                }
            }))
    }
}
