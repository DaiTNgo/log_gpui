use crate::search::SearchState;

use gpui::{
    div, px, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled,
};
use gpui_component::{Icon};
use gpui_kit::assets::IconName;

pub struct MatchNavigator;

impl MatchNavigator {
    /// Render a floating match navigator overlay for the log viewport.
    ///
    /// Returns `None` when search is closed. Otherwise returns an absolutely
    /// positioned element to be placed as a child of the canvas container
    /// (which must be `.relative()`).
    pub fn render(
        search_state: &SearchState,
        view: &gpui::Entity<crate::app::AppView>,
    ) -> Option<impl IntoElement> {
        if !search_state.is_open {
            return None;
        }

        let (curr_idx, total) = search_state.current_match_display();

        let navigator_text = if total == 0 {
            "No matches".to_string()
        } else {
            format!("{} / {}", curr_idx, total)
        };

        let view_prev = view.clone();
        let view_next = view.clone();

        let el = div()
            .id("match_navigator")
            .absolute()
            .top(px(8.0))
            .right(px(24.0))
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .px_2p5()
            .py_1()
            .rounded_md()
            .border_1()
            .shadow_sm()
            .child(
                div()
                    .text_xs()
                    .font_family(".AppleSystemUIFontMonospaced")
                    .child(navigator_text),
            )
            .child(
                div()
                    .id("match_nav_prev_btn")
                    .cursor_pointer()
                    .p_0p5()
                    .rounded_sm()
                    .child(Icon::new(IconName::ChevronUp).size(px(12.0)))
                    .on_click(move |_, _window, cx| {
                        view_prev.update(cx, |this, cx| {
                            this.jump_prev_match(cx);
                        });
                    }),
            )
            .child(
                div()
                    .id("match_nav_next_btn")
                    .cursor_pointer()
                    .p_0p5()
                    .rounded_sm()
                    .child(Icon::new(IconName::ChevronDown).size(px(12.0)))
                    .on_click(move |_, _window, cx| {
                        view_next.update(cx, |this, cx| {
                            this.jump_next_match(cx);
                        });
                    }),
            );

        Some(el)
    }
}
