use crate::density::Density;
use crate::highlight::RowPaint;
use crate::marker::{RulerMark, level_marker};
use crate::model::{LogEntry, LogLevel};
use crate::palette::{HighlightColor, Tone, Weight, text_rgb};
use crate::search::SearchState;
use crate::text_selection::{TextSelection, clip_to_span};
use crate::ui::cells::{Layer, TextCell, cell_text, compose, shows_placeholder};
use gpui::StyledText;
use gpui::{
    AppContext as _, DefiniteLength, ElementId, InteractiveElement, IntoElement,
    ListHorizontalSizingBehavior, MouseButton, ParentElement, Pixels, Render,
    StatefulInteractiveElement, Styled, UniformListScrollHandle, Window, div,
    prelude::FluentBuilder as _, px, uniform_list,
};
use gpui_component::Icon;
use gpui_component::menu::ContextMenuExt as _;
use gpui_component::scroll::ScrollableElement as _;
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

/// Theme colors the table needs, resolved by the caller.
#[derive(Clone, Copy)]
pub struct CanvasColors {
    pub background: gpui::Hsla,
    pub header_bg: gpui::Hsla,
    pub header_fg: gpui::Hsla,
    pub border: gpui::Hsla,
    pub selected: gpui::Hsla,
    pub search: gpui::Hsla,
    /// Paint of the Focus Find matches in the Focus Pane.
    pub find: gpui::Hsla,
    pub muted: gpui::Hsla,
    pub danger: gpui::Hsla,
    pub warning: gpui::Hsla,
    pub info: gpui::Hsla,
    /// Paint of the selected text.
    pub text_selection: gpui::Hsla,
    /// Whether the dark theme is active (picks the palette's dark values).
    pub dark: bool,
}

impl CanvasColors {
    pub fn tone(&self) -> Tone {
        if self.dark { Tone::Dark } else { Tone::Light }
    }

    pub(crate) fn level(&self, level: LogLevel) -> gpui::Hsla {
        match level {
            LogLevel::Fatal | LogLevel::Error => self.danger,
            LogLevel::Warn => self.warning,
            LogLevel::Info => self.info,
            LogLevel::Debug | LogLevel::Verbose | LogLevel::Unknown => self.muted,
        }
    }
}

pub struct CanvasViewState<'a> {
    pub view: gpui::Entity<crate::app::AppView>,
    pub entries: Arc<Vec<LogEntry>>,
    pub visible_indices: Arc<Vec<usize>>,
    pub selected_row_pos: Option<usize>,
    /// The Highlight Rule paint of each entry (by entry position).
    pub paints: Arc<Vec<Option<RowPaint>>>,
    pub selection: Option<TextSelection>,
    /// Receives where the rows are on screen while painting.
    pub bounds_sink: std::rc::Rc<std::cell::Cell<gpui::Bounds<Pixels>>>,
    pub density: Density,
    pub column_layout: &'a crate::column::ColumnLayoutState,
    pub search_state: &'a SearchState,
    pub colors: CanvasColors,
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
            .bg(state.colors.background)
            .child(div().absolute().inset_0().bg(state.colors.header_bg))
            .border_b_1()
            .border_color(state.colors.border)
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
                    .child(div().w(px(64.0)).px_2().text_right().child("#"))
                    .child(div().w(px(24.0)).text_center().child("•")),
            );

        for pinned in pinned_columns {
            let w = state.column_layout.width(*pinned);
            anchored_header =
                anchored_header.child(render_pinnable_column_header(*pinned, w, true, &state));
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
            scrolling_header_cols =
                scrolling_header_cols.child(render_pinnable_column_header(*col, w, false, &state));
        }

        scrolling_header_cols = scrolling_header_cols.child(div().flex_1().px_2().child("MESSAGE"));

        let header = div()
            .h(px(26.0))
            .w_full()
            .relative()
            .overflow_hidden()
            .flex()
            .flex_row()
            .items_center()
            .bg(state.colors.header_bg)
            .text_color(state.colors.header_fg)
            .border_b_1()
            .border_color(state.colors.border)
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
        let longest_vis_pos = visible_clone
            .iter()
            .enumerate()
            .max_by_key(|(_, entry_ix)| {
                entries_clone
                    .get(**entry_ix)
                    .map(|e| e.payload.len())
                    .unwrap_or(0)
            })
            .map(|(vis_pos, _)| vis_pos)
            .unwrap_or(0);
        let env = RowEnv {
            entries: state.entries.clone(),
            visible: state.visible_indices.clone(),
            view: state.view.clone(),
            colors: state.colors,
            selected_pos: state.selected_row_pos,
            search_matcher: state.search_state.matcher.clone(),
            matched_positions: state.search_state.matched_shared.clone(),
            current_match_pos: state.search_state.current_match_pos(),
            scroll_handle: state.scroll_handle.clone(),
            layout: state.column_layout.clone(),
            paints: state.paints.clone(),
            selection: state.selection,
            row_height,
            font_size,
        };

        let list = uniform_list(
            "log_canvas_list",
            total_visible,
            move |range, _window, _cx| {
                range
                    .filter_map(|vis_pos| env.row(vis_pos))
                    .collect::<Vec<_>>()
            },
        )
        .size_full()
        .with_width_from_item(Some(longest_vis_pos))
        .with_horizontal_sizing_behavior(ListHorizontalSizingBehavior::Unconstrained)
        .track_scroll(state.scroll_handle);

        // 3. Overview Ruler (Minimap at right edge - 16px)
        let ruler_marks =
            compute_ruler_marks(&state.entries, &state.visible_indices, state.search_state);

        let mut ruler_elements = div().w(px(16.0)).h_full().border_l_1().relative();

        for mark in &ruler_marks {
            let color = if mark.is_search {
                state.colors.search
            } else if let Some(c) = &mark.color {
                gpui::rgb(c.rgb(state.colors.tone())).into()
            } else {
                state.colors.muted
            };
            let mark_el = if mark.is_current_match {
                div()
                    .absolute()
                    .top(DefiniteLength::Fraction(mark.position))
                    .left(px(2.0))
                    .w(px(12.0))
                    .h(px(5.0))
                    .rounded_xs()
                    .bg(color)
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
                    .bg(color.opacity(if mark.is_search {
                        0.7
                    } else if mark.color.is_some() {
                        1.0
                    } else {
                        0.5
                    }))
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
        // A Marker on the ruler: a click near it scrolls to its line.
        let marker_marks: Vec<(f32, usize)> = ruler_marks
            .iter()
            .filter(|m| m.color.is_some())
            .map(|m| (m.position, m.row_pos))
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

                let near_marker = marker_marks
                    .iter()
                    .min_by(|a, b| {
                        (a.0 - ratio)
                            .abs()
                            .partial_cmp(&(b.0 - ratio).abs())
                            .unwrap()
                    })
                    .copied()
                    .filter(|(pos, _)| (pos - ratio).abs() <= 0.02);
                if let Some((_, row_pos)) = near_marker {
                    view_for_ruler.update(cx, |this, cx| this.jump_to_row(row_pos, cx));
                    return;
                }

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
                    .child({
                        let bounds_sink = state.bounds_sink.clone();
                        let (v_down, v_right, v_move, v_up, v_up_out) = (
                            state.view.clone(),
                            state.view.clone(),
                            state.view.clone(),
                            state.view.clone(),
                            state.view.clone(),
                        );
                        let view_menu = state.view.clone();
                        div()
                            .id("canvas_list_container")
                            .flex_1()
                            .h_full()
                            .overflow_hidden()
                            .relative()
                            .horizontal_scrollbar(state.scroll_handle)
                            .on_mouse_down(MouseButton::Left, move |event, _window, cx| {
                                v_down.update(cx, |this, cx| {
                                    this.start_text_selection(event.position, cx)
                                });
                            })
                            .on_mouse_down(MouseButton::Right, move |event, _window, cx| {
                                v_right.update(cx, |this, cx| {
                                    this.on_cell_right_click(event.position, cx)
                                });
                            })
                            .on_mouse_move(move |event, _window, cx| {
                                if event.pressed_button == Some(MouseButton::Left) {
                                    v_move.update(cx, |this, cx| {
                                        this.drag_text_selection(event.position, cx)
                                    });
                                }
                            })
                            .on_mouse_up(MouseButton::Left, move |event, _window, cx| {
                                v_up.update(cx, |this, cx| {
                                    this.finish_text_selection(event.position, cx)
                                });
                            })
                            .on_mouse_up_out(MouseButton::Left, move |event, _window, cx| {
                                v_up_out.update(cx, |this, cx| {
                                    this.finish_text_selection(event.position, cx)
                                });
                            })
                            .child(
                                gpui::canvas(
                                    |bounds, _, _| bounds,
                                    move |bounds, _, _, _| bounds_sink.set(bounds),
                                )
                                .absolute()
                                .size_full(),
                            )
                            .child(list)
                            .context_menu(move |menu, _window, cx| {
                                crate::ui::cell_menu::cell_menu(menu, &view_menu, cx)
                            })
                    })
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
        PinToggleState::UnpinnedIdle => div().into_any_element(),
        state_variant => {
            if let Some(icon) = state_variant.icon() {
                let btn = div()
                    .id(ElementId::from((
                        if is_sticky {
                            "sticky_pin_toggle"
                        } else {
                            "pin_toggle"
                        },
                        col as usize,
                    )))
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

                btn.child(Icon::new(icon).size_3()).into_any_element()
            } else {
                div().w(px(16.0)).h(px(16.0)).into_any_element()
            }
        }
    };

    let mut header_div = div()
        .id(ElementId::from((
            if is_sticky {
                "sticky_header_col"
            } else {
                "header_col"
            },
            col as usize,
        )))
        .flex_shrink_0()
        .w(width)
        .h_full()
        .relative()
        .flex()
        .flex_row()
        .items_center()
        .overflow_hidden()
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
        .child(div().flex_1().min_w_0().truncate().child(col.label()))
        .child(
            // Overlaid on the right edge so the label keeps the full width
            // while the toggle is hidden.
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .right_1()
                .flex()
                .items_center()
                .child(toggle_element),
        )
        .child(resize_handle)
}

/// Everything one row needs to draw itself, cloned out of the view state so
/// the virtualized list can build rows on its own.
struct RowEnv {
    entries: Arc<Vec<LogEntry>>,
    visible: Arc<Vec<usize>>,
    view: gpui::Entity<crate::app::AppView>,
    colors: CanvasColors,
    selected_pos: Option<usize>,
    search_matcher: Option<crate::search_expr::Matcher>,
    matched_positions: Arc<Vec<usize>>,
    current_match_pos: Option<usize>,
    scroll_handle: UniformListScrollHandle,
    layout: ColumnLayoutState,
    paints: Arc<Vec<Option<RowPaint>>>,
    selection: Option<TextSelection>,
    row_height: f32,
    font_size: f32,
}

impl RowEnv {
    /// Color to paint a rule color with, as an opaque color over `base`.
    fn rule_color(&self, color: &HighlightColor, weight: Weight) -> gpui::Hsla {
        let rgb = color.with_weight(weight, self.colors.tone());
        gpui::rgb(rgb).into()
    }

    fn row(&self, vis_pos: usize) -> Option<gpui::Stateful<gpui::Div>> {
        let colors = self.colors;
        let &entry_ix = self.visible.get(vis_pos)?;
        let entry = self.entries.get(entry_ix)?;
        let paint = self.paints.get(entry_ix).and_then(|p| p.as_ref());

        let level_symbol = if entry.is_continuation() {
            " "
        } else {
            level_marker(entry.level).map_or(" ", |m| m.symbol())
        };

        // Row paint, bottom to top: page → Highlight Rule → selected row →
        // Match of Search. Markers and the text selection paint on the text.
        let base_bg = match paint {
            Some(p) => colors
                .background
                .blend(self.rule_color(&p.color, Weight::Row)),
            None => colors.background,
        };
        let is_match = self.matched_positions.binary_search(&vis_pos).is_ok();
        let is_current_match = is_match && self.current_match_pos == Some(vis_pos);
        let is_selected = self.selected_pos == Some(vis_pos);
        // The selected row and the current Match are marked by an outline
        // drawn over the row (no layout shift), not by a background.
        let row_bg = if is_match && !is_current_match {
            base_bg.blend(colors.search.opacity(0.12))
        } else {
            base_bg
        };
        let outline = if is_current_match {
            Some(colors.search)
        } else if is_selected {
            Some(colors.header_fg)
        } else {
            None
        };
        let search_ranges = match (&self.search_matcher, is_match) {
            (Some(m), true) => m.find_ranges(&entry.payload),
            _ => Vec::new(),
        };
        let selected_in_line = self
            .selection
            .and_then(|s| s.range_in_row(vis_pos, entry.raw_line.len()));

        let anchored_width = self.layout.sticky_partition_width();
        let scroll_x = horizontal_scroll_x(&self.scroll_handle);
        let cell_ctx = CellCtx {
            colors: &colors,
            entry,
            paint,
            search_ranges: &search_ranges,
            find_ranges: &[],
            selected: selected_in_line.as_ref(),
            tone: colors.tone(),
            rule_cell: paint.map(|p| self.rule_color(&p.color, Weight::Cell)),
        };

        let view_for_row = self.view.clone();
        let view_for_gutter = self.view.clone();
        let row_element = div()
            .id(ElementId::from(vis_pos))
            .cursor_pointer()
            .h(px(self.row_height))
            .min_w_full()
            .relative()
            .flex()
            .flex_row()
            .items_center()
            .font_family(".AppleSystemUIFontMonospaced")
            .text_size(px(self.font_size))
            .bg(row_bg)
            .on_click(move |_event, _window, cx| {
                view_for_row.update(cx, |this, cx| this.select_view_row(vis_pos, cx));
            })
            // 1. Horizontally scrolling data columns, shifted by the anchored
            // width so they start next to the Gutter at scroll_x = 0.
            .child({
                let mut scrolling_cols = div().flex().flex_row().items_center().ml(anchored_width);
                for col in &self.layout.unpinned_columns() {
                    scrolling_cols =
                        scrolling_cols.child(render_column_cell(*col, &self.layout, &cell_ctx));
                }
                scrolling_cols
                    .child(
                        div()
                            .flex_1()
                            .px_2()
                            .whitespace_nowrap()
                            .child(render_text_cell(&cell_ctx, TextCell::Message)),
                    )
                    // Breathing space at the right end of unconstrained rows
                    .pr_16()
            })
            // 2. Anchored partition (Gutter + pinned columns), stationary at
            // x = 0. Its opaque background lets the scrolling columns pass
            // underneath.
            .child({
                let is_divider_active = ColumnLayoutState::is_partition_divider_active(scroll_x);
                let total_cover_width = scroll_x + anchored_width;
                div()
                    .id(ElementId::from(("anchored_partition", vis_pos)))
                    .cursor_pointer()
                    .bg(row_bg)
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left_0()
                    .w(total_cover_width)
                    .flex()
                    .flex_row()
                    .items_center()
                    .on_click(move |_event, _window, cx| {
                        view_for_gutter.update(cx, |this, cx| this.select_view_row(vis_pos, cx));
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
                            // Gutter: Level Marker
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .w(px(24.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(colors.level(entry.level))
                                    .child(level_symbol),
                            );
                        for pinned in self.layout.pinned_columns() {
                            inner =
                                inner.child(render_column_cell(*pinned, &self.layout, &cell_ctx));
                        }
                        inner
                    })
            });
        Some(row_element.when_some(outline, |row, color| {
            row.child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .border_1()
                    .border_color(color),
            )
        }))
    }
}

/// What a cell needs to know about its row.
pub(crate) struct CellCtx<'a> {
    pub colors: &'a CanvasColors,
    pub entry: &'a LogEntry,
    pub paint: Option<&'a RowPaint>,
    /// Matches of Search in the message text.
    pub search_ranges: &'a [(usize, usize)],
    /// Matches of Focus Find (painted in the Focus Pane only), in bytes of
    /// the stored line.
    pub find_ranges: &'a [(usize, usize)],
    /// The selected part of the line, in bytes of the stored line.
    pub selected: Option<&'a std::ops::Range<usize>>,
    pub tone: Tone,
    /// Paint of the cell that meets the winning Highlight Rule's condition.
    pub rule_cell: Option<gpui::Hsla>,
}

fn hsla_of(rgb: u32) -> gpui::Hsla {
    gpui::rgb(rgb).into()
}

/// The text of one cell, with Markers, Search Matches and the text selection
/// painted on it. A Continuation Line's other cells show a dim `_`.
pub(crate) fn render_text_cell(ctx: &CellCtx, cell: TextCell) -> gpui::AnyElement {
    if shows_placeholder(ctx.entry, cell) {
        return div()
            .text_color(ctx.colors.muted.opacity(0.5))
            .child("_")
            .into_any_element();
    }
    let Some((text, span)) = cell_text(ctx.entry, cell) else {
        return div().into_any_element();
    };
    let mut layers: Vec<Layer> = Vec::new();
    if matches!(cell, TextCell::Message | TextCell::Raw) {
        for &(s, e) in ctx.search_ranges {
            layers.push(Layer {
                range: s..e,
                background: Some(ctx.colors.search.opacity(0.55)),
                color: None,
            });
        }
    }
    if cell == TextCell::Raw {
        for &(s, e) in ctx.find_ranges {
            layers.push(Layer {
                range: s..e,
                background: Some(ctx.colors.find.opacity(0.6)),
                color: None,
            });
        }
    }
    if let Some(span) = &span {
        for marker in &ctx.entry.markers {
            if let Some(part) = clip_to_span(&marker.range, span) {
                let rgb = marker.color.rgb(ctx.tone);
                layers.push(Layer {
                    range: part.start - span.start..part.end - span.start,
                    background: Some(hsla_of(rgb)),
                    color: Some(hsla_of(text_rgb(marker.color.text_tone(ctx.tone)))),
                });
            }
        }
        if let Some(sel) = ctx.selected {
            if let Some(part) = clip_to_span(sel, span) {
                layers.push(Layer {
                    range: part.start - span.start..part.end - span.start,
                    background: Some(ctx.colors.text_selection),
                    color: None,
                });
            }
        }
    }
    let highlights = compose(text, &layers);
    let rule_cell = ctx
        .paint
        .filter(|p| p.column == cell.column_key())
        .and(ctx.rule_cell);
    let mut el = div();
    if let Some(bg) = rule_cell {
        el = el.bg(bg);
    }
    if highlights.is_empty() {
        el.child(text.to_string()).into_any_element()
    } else {
        el.child(StyledText::new(text.to_string()).with_highlights(highlights))
            .into_any_element()
    }
}

fn render_timestamp_cell(ctx: &CellCtx, width: Pixels) -> impl IntoElement {
    div()
        .flex_shrink_0()
        .w(width)
        .px_2()
        .text_color(ctx.colors.muted)
        .truncate()
        .child(render_text_cell(ctx, TextCell::Timestamp))
}

fn render_level_cell(ctx: &CellCtx, width: Pixels) -> impl IntoElement {
    div()
        .flex_shrink_0()
        .w(width)
        .px_1()
        .text_color(ctx.colors.level(ctx.entry.level))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .truncate()
        .child(render_text_cell(ctx, TextCell::Level))
}

fn render_module_cell(ctx: &CellCtx, width: Pixels) -> impl IntoElement {
    div()
        .w(width)
        .flex_shrink_0()
        .px_2()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .truncate()
        .child(render_text_cell(ctx, TextCell::Module))
}

fn render_source_cell(ctx: &CellCtx, width: Pixels) -> impl IntoElement {
    let source = ctx.entry.source.as_deref().unwrap_or("");
    let mut cell = div()
        .flex_shrink_0()
        .w(width)
        .px_2()
        .text_color(ctx.colors.muted)
        .truncate();
    if let (Some(p), Some(bg)) = (ctx.paint, ctx.rule_cell) {
        if p.column == "source" {
            cell = cell.bg(bg);
        }
    }
    cell.child(source.to_string())
}

fn apply_partition_divider_style<T: gpui::Styled>(element: T, is_active: bool) -> T {
    if is_active {
        element.border_r_1().shadow_sm()
    } else {
        element.shadow_none()
    }
}

fn render_column_cell(
    col: PinnableColumn,
    layout: &ColumnLayoutState,
    ctx: &CellCtx,
) -> impl IntoElement {
    let width = layout.width(col);
    match col {
        PinnableColumn::Timestamp => render_timestamp_cell(ctx, width).into_any_element(),
        PinnableColumn::Level => render_level_cell(ctx, width).into_any_element(),
        PinnableColumn::Module => render_module_cell(ctx, width).into_any_element(),
        PinnableColumn::Source => render_source_cell(ctx, width).into_any_element(),
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

            if let Some(marker) = entry.markers.first() {
                marks.push(RulerMark {
                    position: pos,
                    entry_index: entry_ix,
                    row_pos: vis_pos,
                    color: Some(marker.color.clone()),
                    is_search: false,
                    is_current_match: false,
                });
            } else if !search_state.query.trim().is_empty()
                && search_state
                    .matched_visible_indices
                    .binary_search(&vis_pos)
                    .is_ok()
            {
                let is_current = search_state
                    .matched_visible_indices
                    .get(search_state.current_match_idx)
                    == Some(&vis_pos);
                marks.push(RulerMark {
                    position: pos,
                    entry_index: entry_ix,
                    row_pos: vis_pos,
                    color: None,
                    is_search: true,
                    is_current_match: is_current,
                });
            }
        }
    }

    marks
}
