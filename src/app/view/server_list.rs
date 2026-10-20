//! UI: the list of Servers under their Server Group headings, shared by the
//! Servers dialog, the file picker's Server chooser and the Jump Server
//! chooser. The search field is the caller's; the list only gets its text.

use super::AppView;
use crate::remote::Server;
use crate::remote::server_search::grouped;
use gpui::{InteractiveElement, IntoElement, ParentElement, SharedString, Styled, div};
use gpui::StatefulInteractiveElement as _;
use gpui_component::{ActiveTheme as _, Icon, h_flex, v_flex};
use gpui_kit::assets::IconName;

/// How a list lays out the Servers of one group.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ListLayout {
    /// One Server per line.
    Rows,
    /// Side by side, wrapping (buttons).
    Wrap,
}

impl AppView {
    /// Fold or unfold a Server Group (remembered across runs).
    pub fn toggle_server_group(&mut self, group: &str, cx: &mut gpui::Context<Self>) {
        self.remote.picker_state.toggle_group(group);
        self.save_picker_state();
        cx.notify();
    }

    /// The Servers that match `query` under their Server Group. While
    /// searching every group that has a match is open. `row` draws one
    /// Server.
    pub(super) fn render_server_list(
        &self,
        prefix: &'static str,
        query: &str,
        layout: ListLayout,
        exclude: Option<&str>,
        row: impl Fn(&Server, &mut gpui::Context<Self>) -> gpui::AnyElement,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::AnyElement {
        let servers: Vec<Server> =
            self.remote.servers.servers.iter().filter(|s| Some(s.id.as_str()) != exclude).cloned().collect();
        let theme = cx.theme();
        let (muted, border) = (theme.muted_foreground, theme.border);
        if servers.is_empty() {
            return div().text_sm().text_color(muted).child("No Servers yet.").into_any_element();
        }
        let sections = grouped(&servers, query);
        if sections.is_empty() {
            return div().text_sm().text_color(muted).child("No matching servers").into_any_element();
        }
        let searching = !query.trim().is_empty();
        let mut list = v_flex().gap_1();
        for (ix, section) in sections.iter().enumerate() {
            let key = section.group.clone().unwrap_or_default();
            let collapsed = !searching && self.remote.picker_state.is_group_collapsed(&key);
            let toggle = key.clone();
            list = list.child(
                h_flex()
                    .id(SharedString::from(format!("{prefix}-group-{ix}")))
                    .gap_1()
                    .items_center()
                    .cursor_pointer()
                    .text_xs()
                    .text_color(muted)
                    .hover(|s| s.bg(border.opacity(0.4)))
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle_server_group(&toggle, cx)))
                    .child(
                        Icon::new(if collapsed { IconName::ChevronRight } else { IconName::ChevronDown }).size_3(),
                    )
                    .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(section.title().to_string()))
                    .child(format!("({})", section.servers.len())),
            );
            if collapsed {
                continue;
            }
            let rows: Vec<gpui::AnyElement> = section.servers.iter().map(|s| row(s, cx)).collect();
            list = list.child(match layout {
                ListLayout::Rows => v_flex().gap_1().pl_3().children(rows).into_any_element(),
                ListLayout::Wrap => h_flex().gap_1().flex_wrap().pl_3().children(rows).into_any_element(),
            });
        }
        list.into_any_element()
    }
}
