use crate::marker::RulerMark;
use crate::model::{LogEntry, LogLevel};
use crate::search::SearchState;
use crate::theme::{Density, ZED};
use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, uniform_list, DefiniteLength, ElementId, InteractiveElement,
    IntoElement, ListHorizontalSizingBehavior, ParentElement, StatefulInteractiveElement,
    Styled, UniformListScrollHandle,
};
use gpui_component::scroll::ScrollableElement as _;
use std::sync::Arc;

pub struct CanvasViewState<'a> {
    pub view: gpui::Entity<crate::app::AppView>,
    pub entries: Arc<Vec<LogEntry>>,
    pub visible_indices: Arc<Vec<usize>>,
    pub selected_row_pos: Option<usize>,
    pub density: Density,
    pub word_wrap: bool,
    pub focused_tag: Option<&'a str>,
    pub search_state: &'a SearchState,
    pub scroll_handle: &'a UniformListScrollHandle,
}

pub struct LogCanvas;

impl LogCanvas {
    pub fn render(state: CanvasViewState) -> gpui::AnyElement {
        let total_visible = state.visible_indices.len();
        let row_height = state.density.row_height();
        let font_size = state.density.font_size();

        // 1. Column Headers (Zed Minimalist Header with subtle border divider)
        let header = div()
            .h(px(26.0))
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .bg(ZED.surface_background)
            .border_b_1()
            .border_color(ZED.border_variant)
            .text_xs()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(ZED.text_muted)
            .child(
                div()
                    .w(px(64.0))
                    .px_2()
                    .text_right()
                    .child("#"),
            )
            .child(
                div()
                    .w(px(24.0))
                    .text_center()
                    .child("•"),
            )
            .child(
                div()
                    .w(px(120.0))
                    .px_2()
                    .child("TIMESTAMP"),
            )
            .child(
                div()
                    .w(px(68.0))
                    .px_1()
                    .child("LEVEL"),
            )
            .child(
                div()
                    .w(px(100.0))
                    .px_2()
                    .child("MODULE"),
            )
            .child(
                div()
                    .flex_1()
                    .px_2()
                    .child("MESSAGE"),
            )
            .child(
                div()
                    .w(px(16.0)) // Spacer for overview ruler
                    .child(""),
            );

        // 2. Virtualized List using uniform_list
        let entries_clone = state.entries.clone();
        let visible_clone = state.visible_indices.clone();
        let focused_tag_str = state.focused_tag.map(|s| s.to_string());
        let current_search_vis_pos = state
            .search_state
            .matched_visible_indices
            .get(state.search_state.current_match_idx)
            .copied();
        let search_matched_set: std::collections::HashSet<usize> = state
            .search_state
            .matched_visible_indices
            .iter()
            .copied()
            .collect();
        let selected_row_pos = state.selected_row_pos;
        let word_wrap = state.word_wrap;
        let view = state.view.clone();

        let longest_vis_pos = if !word_wrap {
            visible_clone
                .iter()
                .enumerate()
                .max_by_key(|(_, entry_ix)| {
                    entries_clone.get(**entry_ix).map(|e| e.payload.len()).unwrap_or(0)
                })
                .map(|(vis_pos, _)| vis_pos)
                .unwrap_or(0)
        } else {
            0
        };

        let list = uniform_list(
            "log_canvas_list",
            total_visible,
            move |range, _window, _cx| {
                let mut rows = Vec::with_capacity(range.len());

                for vis_pos in range {
                    let Some(&entry_ix) = visible_clone.get(vis_pos) else {
                        continue;
                    };
                    let Some(entry) = entries_clone.get(entry_ix) else {
                        continue;
                    };

                    let is_selected = selected_row_pos == Some(vis_pos);
                    let is_current_search_match = current_search_vis_pos == Some(vis_pos);
                    let is_search_match = search_matched_set.contains(&vis_pos);

                    let is_dimmed = if let Some(ref tag) = focused_tag_str {
                        !entry.tag.eq_ignore_ascii_case(tag)
                    } else {
                        false
                    };

                    let effective_marker = entry.effective_marker();
                    let (marker_symbol, marker_color) = match effective_marker {
                        Some(m) => (m.symbol(), m.color()),
                        None => (" ", ZED.transparent),
                    };

                    let level_val = entry.level;
                    let view_for_lvl = view.clone();
                    let level_badge = render_level_badge(level_val)
                        .id(ElementId::from(("lvl", vis_pos)))
                        .cursor_pointer()
                        .hover(|s| s.opacity(0.8))
                        .on_click(move |_event, _window, cx| {
                            view_for_lvl.update(cx, |this, cx| {
                                this.add_quick_filter_level(level_val, cx);
                            });
                        });

                    let tag_val = entry.tag.clone();
                    let view_for_tag = view.clone();
                    let tag_element = div()
                        .id(ElementId::from(("mod", vis_pos)))
                        .cursor_pointer()
                        .w(px(100.0))
                        .px_2()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(ZED.text_accent)
                        .truncate()
                        .hover(|s| s.underline())
                        .child(tag_val.clone())
                        .on_click(move |_event, _window, cx| {
                            let tag = tag_val.clone();
                            view_for_tag.update(cx, |this, cx| {
                                this.add_quick_filter_module(&tag, cx);
                            });
                        });

                    let is_expanded = entry.is_expanded;
                    let sub_count = entry.sub_lines.len();
                    let view_for_row = view.clone();

                    let row_element = div()
                        .id(ElementId::from(vis_pos))
                        .cursor_pointer()
                        .h(px(row_height))
                        .min_w_full()
                        .flex()
                        .flex_row()
                        .items_center()
                        .font_family(".AppleSystemUIFontMonospaced")
                        .text_size(px(font_size))
                        .bg(if is_selected {
                            ZED.active_line_bg
                        } else if is_current_search_match {
                            ZED.search_current_bg
                        } else if is_search_match {
                            ZED.search_match_bg
                        } else {
                            ZED.editor_background
                        })
                        .opacity(if is_dimmed { 0.35 } else { 1.0 })
                        .border_l_2()
                        .border_color(if is_selected {
                            ZED.border_focused
                        } else if is_current_search_match {
                            ZED.search_current_border
                        } else {
                            ZED.border_transparent
                        })
                        .hover(|s| {
                            if !is_selected {
                                s.bg(ZED.element_hover)
                            } else {
                                s
                            }
                        })
                        .on_click(move |_event, _window, cx| {
                            view_for_row.update(cx, |this, cx| {
                                this.selected_row_pos = Some(vis_pos);
                                cx.notify();
                            });
                        })
                        // Gutter: Line Number
                        .child(
                            div()
                                .flex_shrink_0()
                                .w(px(64.0))
                                .px_2()
                                .text_right()
                                .text_color(if is_selected {
                                    ZED.gutter_active_line_number
                                } else {
                                    ZED.gutter_line_number
                                })
                                .child(format!("{}", entry.index)),
                        )
                        // Gutter: Marker Icon
                        .child(
                            div()
                                .flex_shrink_0()
                                .w(px(24.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_xs()
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_color(marker_color)
                                .child(marker_symbol),
                        )
                        // Column: Timestamp
                        .child(
                            div()
                                .flex_shrink_0()
                                .w(px(120.0))
                                .px_2()
                                .text_color(ZED.text_muted)
                                .truncate()
                                .child(entry.time_str.clone()),
                        )
                        // Column: Level (Interactive quick-filter)
                        .child(
                            div()
                                .flex_shrink_0()
                                .w(px(68.0))
                                .px_1()
                                .child(level_badge),
                        )
                        // Column: Module / Tag (Interactive quick-filter)
                        .child(tag_element.flex_shrink_0())
                        // Column: Message Payload with horizontal scroll support
                        .child(
                            div()
                                .flex_1()
                                .px_2()
                                .text_color(match entry.level {
                                    LogLevel::Fatal | LogLevel::Error => ZED.error,
                                    LogLevel::Warn => ZED.warning,
                                    _ => ZED.text,
                                })
                                .when(!word_wrap, |s| s.whitespace_nowrap())
                                .child(entry.payload.clone()),
                        )
                        // Breathing space at the right end of unconstrained rows
                        .when(!word_wrap, |s| s.pr_16())
                        // Multi-line stack trace toggle indicator if present
                        .child(if sub_count > 0 {
                            let view_for_fold = view.clone();
                            div()
                                .flex_shrink_0()
                                .id(ElementId::from(("fold", vis_pos)))
                                .cursor_pointer()
                                .px_2()
                                .text_xs()
                                .text_color(if is_expanded {
                                    ZED.text_accent
                                } else {
                                    ZED.text_muted
                                })
                                .hover(|s| s.text_color(ZED.text))
                                .child(if is_expanded {
                                    format!("[{} lines] ∨", sub_count)
                                } else {
                                    format!("[{} lines] ›", sub_count)
                                })
                                .on_click(move |_event, _window, cx| {
                                    view_for_fold.update(cx, |this, cx| {
                                        this.toggle_row_expansion(entry_ix, cx);
                                    });
                                })
                                .into_any_element()
                        } else {
                            div().into_any_element()
                        });

                    rows.push(row_element);
                }

                rows
            },
        )
        .size_full()
        .with_width_from_item(Some(longest_vis_pos))
        .with_horizontal_sizing_behavior(if word_wrap {
            ListHorizontalSizingBehavior::FitList
        } else {
            ListHorizontalSizingBehavior::Unconstrained
        })
        .track_scroll(state.scroll_handle);

        // 3. Overview Ruler (Minimap at right edge - 16px)
        let ruler_marks = compute_ruler_marks(&state.entries, &state.visible_indices, state.search_state);

        let mut ruler_elements = div()
            .w(px(16.0))
            .h_full()
            .bg(ZED.surface_background)
            .border_l_1()
            .border_color(ZED.border_variant)
            .relative();

        for mark in ruler_marks {
            let mark_el = div()
                .absolute()
                .top(DefiniteLength::Fraction(mark.position))
                .left(px(3.0))
                .w(px(10.0))
                .h(px(3.0))
                .rounded_xs()
                .bg(mark.color);

            ruler_elements = ruler_elements.child(mark_el);
        }

        // Clickable surface on overview ruler to jump to line
        let view_for_ruler = state.view.clone();
        let ruler_interactive = div()
            .id("overview_ruler")
            .cursor_pointer()
            .w(px(16.0))
            .h_full()
            .child(ruler_elements)
            .on_click(move |event: &gpui::ClickEvent, window, cx| {
                let click_y = event.position().y;
                let vp_height = window.viewport_size().height;
                let header_offset = px(80.0);
                let footer_offset = px(24.0);
                let usable_height = vp_height - header_offset - footer_offset;
                let ratio = if usable_height > px(0.0) {
                    ((click_y - header_offset) / usable_height).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                view_for_ruler.update(cx, |this, cx| {
                    let total = this.visible_indices.len();
                    if total > 0 {
                        let target_row = ((total as f32) * ratio) as usize;
                        this.jump_to_row(target_row.min(total.saturating_sub(1)), cx);
                    }
                });
            });

        // 4. Combined Canvas Container
        div()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .bg(ZED.editor_background)
            .child(header)
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_row()
                    .child(
                        div()
                            .id("canvas_list_container")
                            .flex_1()
                            .h_full()
                            .overflow_hidden()
                            .horizontal_scrollbar(state.scroll_handle)
                            .child(list),
                    )
                    .child(ruler_interactive),
            )
            .into_any_element()
    }
}

fn render_level_badge(level: LogLevel) -> gpui::Div {
    let (fg, bg, border) = match level {
        LogLevel::Fatal => (ZED.error, ZED.error_bg, ZED.error_border),
        LogLevel::Error => (ZED.error, ZED.error_bg, ZED.error_border),
        LogLevel::Warn => (ZED.warning, ZED.warning_bg, ZED.warning_border),
        LogLevel::Info => (ZED.info, ZED.info_bg, ZED.info_border),
        LogLevel::Debug => (ZED.debug, ZED.debug_bg, ZED.border_variant),
        LogLevel::Verbose => (ZED.text_muted, ZED.element_background, ZED.border_variant),
        LogLevel::Unknown => (ZED.text_disabled, ZED.element_background, ZED.border_variant),
    };

    div()
        .px_1p5()
        .py_0p5()
        .rounded_xs()
        .border_1()
        .border_color(border)
        .text_xs()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(fg)
        .bg(bg)
        .child(level.short_code())
}

fn compute_ruler_marks(
    entries: &[LogEntry],
    visible_indices: &[usize],
    search_state: &SearchState,
) -> Vec<RulerMark> {
    let mut marks = Vec::new();
    let total = entries.len();
    if total == 0 {
        return marks;
    }

    for (vis_pos, &entry_ix) in visible_indices.iter().enumerate() {
        if let Some(entry) = entries.get(entry_ix) {
            let pos = (entry_ix as f32 / total as f32).clamp(0.0, 1.0);

            if let Some(marker) = entry.effective_marker() {
                marks.push(RulerMark {
                    position: pos,
                    entry_index: entry_ix,
                    color: marker.color(),
                    marker_type: Some(marker),
                    is_search: false,
                });
            } else if !search_state.query.trim().is_empty()
                && search_state.matched_visible_indices.contains(&vis_pos)
            {
                marks.push(RulerMark {
                    position: pos,
                    entry_index: entry_ix,
                    color: ZED.search_current_border,
                    marker_type: None,
                    is_search: true,
                });
            }
        }
    }

    marks
}
