//! UI: working-set tab bar built from gpui-kit `TabBar` / `Tab`.

use super::AppView;
use crate::workspace::working_set::TabTarget;
use gpui::prelude::FluentBuilder as _;
use gpui::{div, px, MouseButton, Context, InteractiveElement as _, IntoElement, ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled};
use gpui_component::{h_flex, ActiveTheme as _, Icon};
use gpui_kit::assets::IconName;

/// Matches the Zed tab strip height.
const TAB_BAR_HEIGHT: f32 = 34.0;

impl AppView {
    pub(super) fn render_tab_bar(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let view = cx.entity();
        let (labels, active_idx): (Vec<String>, Option<usize>) = match self.active_project() {
            Some(proj) => (
                proj.working_set
                    .tabs()
                    .iter()
                    .map(|tab| match &tab.target {
                        TabTarget::LogSpace { space_id } => proj
                            .log_space(space_id)
                            .map(|s| s.name.clone())
                            .unwrap_or_else(|| space_id.clone()),
                        TabTarget::FileView { space_id, file_id } => {
                            let space = proj.log_space(space_id);
                            let file = space.and_then(|s| s.file(file_id));
                            match (file, space) {
                                (Some(f), Some(s)) => format!("{} · {}", f.display_name, s.name),
                                _ => file_id.clone(),
                            }
                        }
                        TabTarget::InvestigationBoard { case_id } => format!("⚑ {}", case_id),
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
        let tabs = labels.into_iter().enumerate().map(|(index, label)| {
            let active = active_idx == Some(index);
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
                .child(label)
                .child(
                    div()
                        .id(("close-tab", index))
                        .flex()
                        .items_center()
                        .justify_center()
                        .size_4()
                        .rounded_sm()
                        .when(!active, |this| this.invisible())
                        .group_hover(group, |s| s.visible())
                        .hover(|s| s.bg(border))
                        .child(Icon::new(IconName::Close).size_3())
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
