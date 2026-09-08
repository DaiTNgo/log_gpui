use crate::search::SearchState;
use crate::theme::ZED;
use gpui::{
    div, px, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled,
};
use gpui_component::input::{Input, InputState};
use gpui_component::{Icon, IconName};

pub struct SearchOverlay;

impl SearchOverlay {
    pub fn render(
        search_state: &SearchState,
        search_input: &gpui::Entity<InputState>,
        cx: &mut gpui::Context<crate::app::AppView>,
    ) -> gpui::AnyElement {
        if !search_state.is_open {
            return div().into_any_element();
        }

        let (curr_idx, total) = search_state.current_match_display();
        let match_text = if total == 0 {
            if search_state.query.trim().is_empty() {
                "".to_string()
            } else {
                "0 / 0".to_string()
            }
        } else {
            format!("{} / {}", curr_idx, total)
        };

        let case_active = search_state.case_sensitive;
        let regex_active = search_state.is_regex;
        let word_active = search_state.whole_word;

        div()
            .absolute()
            .top(px(42.0))
            .right(px(32.0))
            .w(px(360.0))
            .bg(ZED.elevated_surface)
            .border_1()
            .border_color(ZED.border)
            .rounded_md()
            .shadow_lg()
            .p_2()
            .flex()
            .flex_col()
            .gap_1p5()
            .child(
                // Input row
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_color(ZED.icon_muted)
                            .child(Icon::new(IconName::Search).size(px(14.0))),
                    )
                    .child(
                        div()
                            .flex_1()
                            .child(Input::new(search_input)),
                    )
                    .child(
                        div()
                            .id("close_search_btn")
                            .cursor_pointer()
                            .p_1()
                            .rounded_md()
                            .text_color(ZED.icon_muted)
                            .hover(|s| s.bg(ZED.ghost_hover).text_color(ZED.icon_hover))
                            .child(Icon::new(IconName::Close).size(px(13.0)))
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.toggle_search(cx);
                            })),
                    ),
            )
            .child(
                // Control row: mode toggles + counter + nav arrows
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .pt_1()
                    .child(
                        // Mode toggles: Aa, .*, ↔
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1()
                            .child(
                                div()
                                    .id("toggle_case_btn")
                                    .cursor_pointer()
                                    .px_1p5()
                                    .py_0p5()
                                    .rounded_xs()
                                    .border_1()
                                    .border_color(if case_active {
                                        ZED.border_focused
                                    } else {
                                        ZED.border_variant
                                    })
                                    .text_xs()
                                    .font_family(".AppleSystemUIFontMonospaced")
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .bg(if case_active {
                                        ZED.element_selected
                                    } else {
                                        ZED.element_background
                                    })
                                    .text_color(if case_active {
                                        ZED.text_accent
                                    } else {
                                        ZED.text_muted
                                    })
                                    .hover(|s| s.bg(ZED.ghost_hover))
                                    .child("Aa")
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        this.search_state.toggle_case();
                                        this.run_search(cx);
                                    })),
                            )
                            .child(
                                div()
                                    .id("toggle_regex_btn")
                                    .cursor_pointer()
                                    .px_1p5()
                                    .py_0p5()
                                    .rounded_xs()
                                    .border_1()
                                    .border_color(if regex_active {
                                        ZED.border_focused
                                    } else {
                                        ZED.border_variant
                                    })
                                    .text_xs()
                                    .font_family(".AppleSystemUIFontMonospaced")
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .bg(if regex_active {
                                        ZED.element_selected
                                    } else {
                                        ZED.element_background
                                    })
                                    .text_color(if regex_active {
                                        ZED.text_accent
                                    } else {
                                        ZED.text_muted
                                    })
                                    .hover(|s| s.bg(ZED.ghost_hover))
                                    .child(".*")
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        this.search_state.toggle_regex();
                                        this.run_search(cx);
                                    })),
                            )
                            .child(
                                div()
                                    .id("toggle_word_btn")
                                    .cursor_pointer()
                                    .px_1p5()
                                    .py_0p5()
                                    .rounded_xs()
                                    .border_1()
                                    .border_color(if word_active {
                                        ZED.border_focused
                                    } else {
                                        ZED.border_variant
                                    })
                                    .text_xs()
                                    .font_family(".AppleSystemUIFontMonospaced")
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .bg(if word_active {
                                        ZED.element_selected
                                    } else {
                                        ZED.element_background
                                    })
                                    .text_color(if word_active {
                                        ZED.text_accent
                                    } else {
                                        ZED.text_muted
                                    })
                                    .hover(|s| s.bg(ZED.ghost_hover))
                                    .child("↔")
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        this.search_state.toggle_whole_word();
                                        this.run_search(cx);
                                    })),
                            ),
                    )
                    .child(
                        // Counter & Navigation arrows
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1p5()
                            .child(
                                div()
                                    .text_xs()
                                    .font_family(".AppleSystemUIFontMonospaced")
                                    .text_color(if total > 0 {
                                        ZED.text
                                    } else {
                                        ZED.text_muted
                                    })
                                    .child(match_text),
                            )
                            .child(
                                div()
                                    .id("prev_match_btn")
                                    .cursor_pointer()
                                    .p_1()
                                    .rounded_md()
                                    .hover(|s| s.bg(ZED.ghost_hover).text_color(ZED.icon_hover))
                                    .text_color(ZED.icon_muted)
                                    .child(Icon::new(IconName::ChevronUp).size(px(12.0)))
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        this.jump_prev_match(cx);
                                    })),
                            )
                            .child(
                                div()
                                    .id("next_match_btn")
                                    .cursor_pointer()
                                    .p_1()
                                    .rounded_md()
                                    .hover(|s| s.bg(ZED.ghost_hover).text_color(ZED.icon_hover))
                                    .text_color(ZED.icon_muted)
                                    .child(Icon::new(IconName::ChevronDown).size(px(12.0)))
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        this.jump_next_match(cx);
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }
}
