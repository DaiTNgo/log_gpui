use crate::marker::RulerMark;
use crate::model::{LogEntry, LogLevel};
use crate::search::SearchState;
use crate::density::Density;
use gpui::{
    div, prelude::FluentBuilder as _, px, uniform_list, AppContext as _, DefiniteLength,
    ElementId, InteractiveElement, IntoElement, ListHorizontalSizingBehavior, MouseButton,
    ParentElement, Pixels, Render, StatefulInteractiveElement, Styled, UniformListScrollHandle,
    Window,
};
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::Icon;
use std::sync::Arc;

use crate::column::{ColumnLayoutState, PinToggleState, PinnableColumn};


#[derive(Clone)]
pub struct ResizeColumn {
    pub col: PinnableColumn,
}

impl Render for ResizeColumn {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
    }
}
/// Helper to extract horizontal scroll offset from UniformListScrollHandle.
#[inline]
pub fn horizontal_scroll_x(scroll_handle: &UniformListScrollHandle) -> Pixels {
    (-scroll_handle.0.borrow().base_handle.offset().x).max(px(0.0))
}

pub struct CanvasViewState<'a> {
    pub view: gpui::Entity<crate::app::AppView>,
    pub entries: Arc<Vec<LogEntry>>,
    pub visible_indices: Arc<Vec<usize>>,
    pub selected_row_pos: Option<usize>,
    pub density: Density,
    pub column_layout: &'a crate::column::ColumnLayoutState,
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

        let gutter_width = state.column_layout.gutter_width();
        let scroll_x = horizontal_scroll_x(state.scroll_handle);

        let sticky_partition_width = state.column_layout.sticky_partition_width();
        let is_divider_active = ColumnLayoutState::is_partition_divider_active(scroll_x);
        let pinned_columns = state.column_layout.pinned_columns();
        let unpinned_columns = state.column_layout.unpinned_columns();

        // 1. Column Headers (Zed Minimalist Header with Unified Sticky Partition)
        // Pinned columns anchor to the left margin in schema order
        // Unpinned columns and MESSAGE scroll horizontally underneath
        let anchored_width = sticky_partition_width;
        let mut anchored_header = div()
            .absolute()
            .top_0()
            .bottom_0()
            .left_0()
            .w(anchored_width)
            .flex()
            .flex_row()
            .items_center()
            .border_b_1()
            .map(|d| apply_partition_divider_style(d, is_divider_active))
            .child(
                // Stationary Gutter Header
                div()
                    .flex_shrink_0()
                    .w(gutter_width)
                    .h_full()
                    .flex()
                    .flex_row()
                    .items_center()
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
                    ),
            );

        for pinned in pinned_columns {
            let w = state.column_layout.width(*pinned);
            anchored_header = anchored_header.child(
                render_pinnable_column_header(*pinned, w, true, &state),
            );
        }

        let mut scrolling_header_cols = div()
            .flex()
            .flex_row()
            .items_center()
            .relative()
            .left(-scroll_x)
            .ml(anchored_width);

        for col in &unpinned_columns {
            let w = state.column_layout.width(*col);
            scrolling_header_cols = scrolling_header_cols.child(
                render_pinnable_column_header(*col, w, false, &state),
            );
        }

        scrolling_header_cols = scrolling_header_cols.child(
            div()
                .flex_1()
                .px_2()
                .child("MESSAGE"),
        );

        let header = div()
            .h(px(26.0))
            .w_full()
            .relative()
            .overflow_hidden()
            .flex()
            .flex_row()
            .items_center()
            .border_b_1()
            .text_xs()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .child(
                // Base scrolling columns container
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_row()
                    .items_center()
                    .child(scrolling_header_cols),
            )
            .child(
                div()
                    .w(px(16.0)) // Spacer for overview ruler
                    .child(""),
            )
            .child(anchored_header);

        // 2. Virtualized List using uniform_list
        let entries_clone = state.entries.clone();
        let visible_clone = state.visible_indices.clone();
        let focused_tag_str = state.focused_tag.map(|s| s.to_string());
        let view = state.view.clone();
        let scroll_handle_for_rows = state.scroll_handle.clone();
        let column_layout_for_rows = state.column_layout.clone();
        let longest_vis_pos = visible_clone
            .iter()
            .enumerate()
            .max_by_key(|(_, entry_ix)| {
                entries_clone.get(**entry_ix).map(|e| e.payload.len()).unwrap_or(0)
            })
            .map(|(vis_pos, _)| vis_pos)
            .unwrap_or(0);

        let list = uniform_list(
            "log_canvas_list",
            total_visible,
            move |range, _window, _cx| {
                let scroll_x = horizontal_scroll_x(&scroll_handle_for_rows);
                let anchored_width = column_layout_for_rows.sticky_partition_width();
                let unpinned_columns_for_rows = column_layout_for_rows.unpinned_columns();
                let mut rows = Vec::with_capacity(range.len());
                for vis_pos in range {
                    let Some(&entry_ix) = visible_clone.get(vis_pos) else {
                        continue;
                    };
                    let Some(entry) = entries_clone.get(entry_ix) else {
                        continue;
                    };

                    let is_dimmed = if let Some(ref tag) = focused_tag_str {
                        !entry.tag.eq_ignore_ascii_case(tag)
                    } else {
                        false
                    };

                    let effective_marker = entry.effective_marker();
                    let marker_symbol = match effective_marker {
                        Some(m) => m.symbol(),
                        None => " ",
                    };


                    let is_expanded = entry.is_expanded;
                    let sub_count = entry.sub_lines.len();

                    let view_for_row = view.clone();
                    let view_for_gutter = view.clone();

                    let row_element = div()
                        .id(ElementId::from(vis_pos))
                        .cursor_pointer()
                        .h(px(row_height))
                        .min_w_full()
                        .relative()
                        .flex()
                        .flex_row()
                        .items_center()
                        .font_family(".AppleSystemUIFontMonospaced")
                        .text_size(px(font_size))
                        .opacity(if is_dimmed { 0.35 } else { 1.0 })
                        .on_click(move |_event, _window, cx| {
                            view_for_row.update(cx, |this, cx| {
                                this.selected_row_pos = Some(vis_pos);
                                cx.notify();
                            });
                        })
                        // 1. Horizontally Scrolling Data Columns
                        // Shifted by gutter_width so columns start adjacent to Gutter at scroll_x = 0
                        .child({
                            let mut scrolling_cols = div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .ml(anchored_width);

                            for col in &unpinned_columns_for_rows {
                                scrolling_cols = scrolling_cols.child(render_column_cell(
                                    *col,
                                    &column_layout_for_rows,
                                    entry,
                                    vis_pos,
                                    view.clone(),
                                    false,
                                ));
                            }

                            scrolling_cols
                                // Column: Message Payload (Unconstrained single-line layout)
                                .child(
                                    div()
                                        .flex_1()
                                        .px_2()
                                        .whitespace_nowrap()
                                        .child(entry.payload.clone()),
                                )
                                // Breathing space at the right end of unconstrained rows
                                .pr_16()
                                // Multi-line stack trace toggle indicator if present
                                .child(if sub_count > 0 {
                                    let view_for_fold = view.clone();
                                    div()
                                        .flex_shrink_0()
                                        .id(ElementId::from(("fold", vis_pos)))
                                        .cursor_pointer()
                                        .px_2()
                                        .text_xs()
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
                                })
                        })
                        // 2. Anchored Partition (Stationary at x = 0 with Gutter + Sticky Columns + Partition Divider)
                        // Absolute positioning with left = scroll_x counteracts the row's horizontal offset.
                        // Opaque row_bg ensures scrolling columns pass cleanly underneath.
                        .child({
                            let is_divider_active = ColumnLayoutState::is_partition_divider_active(scroll_x);
                            let pinned_columns = column_layout_for_rows.pinned_columns();
                            let total_cover_width = scroll_x + anchored_width;

                            div()
                                .id(ElementId::from(("anchored_partition", vis_pos)))
                                .cursor_pointer()
                                .absolute()
                                .top_0()
                                .bottom_0()
                                .left_0()
                                .w(total_cover_width)
                                .flex()
                                .flex_row()
                                .items_center()
                                .on_click(move |_event, _window, cx| {
                                    view_for_gutter.update(cx, |this, cx| {
                                        this.selected_row_pos = Some(vis_pos);
                                        cx.notify();
                                    });
                                })
                                .child({
                                    let mut inner = div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .h_full()
                                        .ml(scroll_x)
                                        .w(anchored_width)
                                        .border_l_2()
                                        .map(|d| apply_partition_divider_style(d, is_divider_active))
                                        // Gutter: Line Number
                                        .child(
                                            div()
                                                .flex_shrink_0()
                                                .w(px(64.0))
                                                .px_2()
                                                .text_right()
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
                                                .child(marker_symbol),
                                        );

                                    for pinned in pinned_columns {
                                        inner = inner.child(render_column_cell(
                                            *pinned,
                                            &column_layout_for_rows,
                                            entry,
                                            vis_pos,
                                            view.clone(),
                                            true,
                                        ));
                                    }

                                    inner
                                })
                        });

                    rows.push(row_element);
                }

                rows
            },
        )
        .size_full()
        .with_width_from_item(Some(longest_vis_pos))
        .with_horizontal_sizing_behavior(ListHorizontalSizingBehavior::Unconstrained)
        .track_scroll(state.scroll_handle);

        // 3. Overview Ruler (Minimap at right edge - 16px)
        let ruler_marks = compute_ruler_marks(&state.entries, &state.visible_indices, state.search_state);

        let mut ruler_elements = div()
            .w(px(16.0))
            .h_full()
            .border_l_1()
            .relative();

        for mark in &ruler_marks {
            let mark_el = if mark.is_current_match {
                div()
                    .absolute()
                    .top(DefiniteLength::Fraction(mark.position))
                    .left(px(2.0))
                    .w(px(12.0))
                    .h(px(5.0))
                    .rounded_xs()
                    .border_1()
                    .shadow_sm()
            } else {
                div()
                    .absolute()
                    .top(DefiniteLength::Fraction(mark.position))
                    .left(px(3.0))
                    .w(px(10.0))
                    .h(px(3.0))
                    .rounded_xs()
            };

            ruler_elements = ruler_elements.child(mark_el);
        }

        // Clickable surface on overview ruler to jump to line
        // or jump to the nearest search match when a query is active.
        let view_for_ruler = state.view.clone();
        let search_marks: Vec<(f32, usize)> = ruler_marks
            .iter()
            .filter(|m| m.is_search)
            .map(|m| (m.position, m.entry_index))
            .collect();
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

                if !search_marks.is_empty() {
                    let nearest = search_marks
                        .iter()
                        .min_by(|a, b| {
                            (a.0 - ratio)
                                .abs()
                                .partial_cmp(&(b.0 - ratio).abs())
                                .unwrap()
                        })
                        .copied();
                    if let Some((_, entry_ix)) = nearest {
                        view_for_ruler.update(cx, |this, cx| {
                            this.jump_to_match_by_entry(entry_ix, cx);
                        });
                        return;
                    }
                }

                view_for_ruler.update(cx, |this, cx| {
                    let total = this.visible_indices.len();
                    if total > 0 {
                        let target_row = ((total as f32) * ratio) as usize;
                        this.jump_to_row(target_row.min(total.saturating_sub(1)), cx);
                    }
                });
            });

        // 5. Combined Canvas Container
        div()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .child(header)
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_row()
                    .relative()
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

fn render_pinnable_column_header(
    col: PinnableColumn,
    width: Pixels,
    is_sticky: bool,
    state: &CanvasViewState,
) -> impl IntoElement {
    let toggle_state = state.column_layout.pin_toggle_state(col);
    let view_header = state.view.clone();
    let view_toggle = state.view.clone();
    let view_click = state.view.clone();

    // In UnpinnedIdle state, render a non-interactive spacer so it never intercepts clicks
    let toggle_element = match toggle_state {
        PinToggleState::UnpinnedIdle => div().w(px(16.0)).h(px(16.0)).into_any_element(),
        state_variant => {
            if let Some(icon) = state_variant.icon() {
                
                let btn = div()
                    .id(ElementId::from((if is_sticky { "sticky_pin_toggle" } else { "pin_toggle" }, col as usize)))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(16.0))
                    .h(px(16.0))
                    .rounded_md()
                    .on_hover(move |is_hovered, _window, cx| {
                        view_toggle.update(cx, |this, cx| {
                            this.set_column_toggle_hovered(col, *is_hovered, cx);
                        });
                    })
                    .on_click(move |_event, _window, cx| {
                        cx.stop_propagation();
                        view_click.update(cx, |this, cx| {
                            this.toggle_pin_column(col, cx);
                        });
                    });

                btn.child(
                    Icon::new(icon).size_3(),
                )
                .into_any_element()
            } else {
                div().w(px(16.0)).h(px(16.0)).into_any_element()
            }
        }
    };

    let mut header_div = div()
        .id(ElementId::from((if is_sticky { "sticky_header_col" } else { "header_col" }, col as usize)))
        .flex_shrink_0()
        .w(width)
        .h_full()
        .relative()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .on_hover(move |is_hovered, _window, cx| {
            view_header.update(cx, |this, cx| {
                this.set_column_header_hovered(col, *is_hovered, cx);
            });
        });

    if col.is_compact() {
        header_div = header_div.px_1();
    } else {
        header_div = header_div.px_2();
    }

    let view_resize = state.view.clone();
    let view_drag_move = state.view.clone();
    let view_drag_end_up = state.view.clone();
    let view_drag_end_out = state.view.clone();

    let resize_handle = div()
        .id(ElementId::from(("col_resize_handle", col as usize)))
        .occlude()
        .cursor_col_resize()
        .absolute()
        .top_0()
        .bottom_0()
        .right_0()
        .w(px(6.0))
        .h_full()
        .on_drag(ResizeColumn { col }, |drag, _point, _window, cx| {
            cx.stop_propagation();
            cx.new(|_| drag.clone())
        })
        .on_drag_move(move |e: &gpui::DragMoveEvent<ResizeColumn>, _window, cx| {
            view_drag_move.update(cx, |this, cx| {
                this.update_column_resize(e.event.position.x, cx);
            });
        })
        .on_mouse_down(MouseButton::Left, move |event, _window, cx| {
            cx.stop_propagation();
            if event.click_count >= 2 {
                view_resize.update(cx, |this, cx| {
                    this.auto_fit_column(col, cx);
                });
            } else {
                view_resize.update(cx, |this, cx| {
                    this.start_resizing_column(col, event.position.x, cx);
                });
            }
        })
        .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
            view_drag_end_up.update(cx, |this, cx| {
                this.finish_resizing_column(cx);
            });
        })
        .on_mouse_up_out(MouseButton::Left, move |_event, _window, cx| {
            view_drag_end_out.update(cx, |this, cx| {
                this.finish_resizing_column(cx);
            });
        })
        .on_click(move |_event, _window, cx| {
            cx.stop_propagation();
        });

    header_div
        .child(
            div()
                .truncate()
                .child(col.label()),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .mr_1()
                .child(toggle_element),
        )
        .child(resize_handle)
}

fn render_timestamp_cell(time_str: &str, width: Pixels) -> impl IntoElement {
    div()
        .flex_shrink_0()
        .w(width)
        .px_2()
        .truncate()
        .child(time_str.to_string())
}

fn render_level_cell(
    level_val: LogLevel,
    width: Pixels,
    id_prefix: &'static str,
    vis_pos: usize,
    view: gpui::Entity<crate::app::AppView>,
) -> impl IntoElement {
    div()
        .flex_shrink_0()
        .w(width)
        .px_1()
        .child(
            render_level_badge(level_val)
                .id(ElementId::from((id_prefix, vis_pos)))
                .cursor_pointer()
                .hover(move |s| s.opacity(0.8))
                .on_click(move |_event, _window, cx| {
                    view.update(cx, |this, cx| {
                        this.add_quick_filter_level(level_val, cx);
                    });
                }),
        )
}

fn render_module_cell(
    tag_val: &str,
    width: Pixels,
    id_prefix: &'static str,
    vis_pos: usize,
    view: gpui::Entity<crate::app::AppView>,
) -> impl IntoElement {
    let tag = tag_val.to_string();
    div()
        .id(ElementId::from((id_prefix, vis_pos)))
        .cursor_pointer()
        .w(width)
        .flex_shrink_0()
        .px_2()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .truncate()
        .hover(move |s| s.underline())
        .child(tag.clone())
        .on_click(move |_event, _window, cx| {
            let t = tag.clone();
            view.update(cx, |this, cx| {
                this.add_quick_filter_module(&t, cx);
            });
        })
}

fn render_level_badge(level: LogLevel) -> gpui::Div {

    div()
        .px_1p5()
        .py_0p5()
        .rounded_xs()
        .border_1()
        .text_xs()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .child(level.short_code())
}

fn apply_partition_divider_style<T: gpui::Styled>(element: T, is_active: bool) -> T {
    if is_active {
        element
            .border_r_1()
            .shadow_sm()
    } else {
        element.shadow_none()
    }
}

fn render_column_cell(
    col: PinnableColumn,
    layout: &ColumnLayoutState,
    entry: &LogEntry,
    vis_pos: usize,
    view: gpui::Entity<crate::app::AppView>,
    is_sticky: bool,
) -> impl IntoElement {
    let id_prefix = if is_sticky {
        match col {
            PinnableColumn::Timestamp => "sticky_ts",
            PinnableColumn::Level => "sticky_lvl",
            PinnableColumn::Module => "sticky_mod",
        }
    } else {
        match col {
            PinnableColumn::Timestamp => "ts",
            PinnableColumn::Level => "lvl",
            PinnableColumn::Module => "mod",
        }
    };
    let width = layout.width(col);
    match col {
        PinnableColumn::Timestamp => render_timestamp_cell(&entry.time_str, width).into_any_element(),
        PinnableColumn::Level => {
            render_level_cell(entry.level, width, id_prefix, vis_pos, view).into_any_element()
        }
        PinnableColumn::Module => {
            render_module_cell(&entry.tag, width, id_prefix, vis_pos, view).into_any_element()
        }
    }
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
                    marker_type: Some(marker),
                    is_search: false,
                    is_current_match: false,
                });
            } else if !search_state.query.trim().is_empty()
                && search_state.matched_visible_indices.contains(&vis_pos)
            {
                let is_current = search_state
                    .matched_visible_indices
                    .get(search_state.current_match_idx)
                    == Some(&vis_pos);
                marks.push(RulerMark {
                    position: pos,
                    entry_index: entry_ix,
                    marker_type: None,
                    is_search: true,
                    is_current_match: is_current,
                });
            }
        }
    }

    marks
}
