use crate::density::Density;
use crate::highlight::RowPaint;
use crate::marker::level_marker;
use crate::model::{LogEntry, LogLevel};
use crate::palette::{HighlightColor, Tone, Weight, text_rgb};
use crate::search::SearchState;
use crate::text_selection::{TextSelection, clip_to_span};
use crate::ui::cells::{Layer, TextCell, cell_text, compose, shows_placeholder};
use gpui_component::ActiveTheme as _;
use gpui::StyledText;
use gpui::{
    AppContext as _, ElementId, InteractiveElement, IntoElement, ListHorizontalSizingBehavior,
    MouseButton, ParentElement, Pixels, Render, StatefulInteractiveElement, Styled,
    UniformListScrollHandle, Window, div, prelude::FluentBuilder as _, px, uniform_list,
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
    /// Color of the line number of a stderr line (not the color of a level).
    pub stderr: gpui::Hsla,
    /// Paint of the selected text.
    pub text_selection: gpui::Hsla,
    /// Whether the dark theme is active (picks the palette's dark values).
    pub dark: bool,
}

/// The color the line number of a stderr line is drawn in: a soft red that is
/// not the red of the Level Marker.
pub fn stderr_color(dark: bool) -> gpui::Hsla {
    if dark { gpui::hsla(8.0 / 360.0, 0.78, 0.72, 1.0) } else { gpui::hsla(8.0 / 360.0, 0.62, 0.46, 1.0) }
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
    /// The rows, borrowed from the view for this frame only (never cloned).
    pub entries: &'a [LogEntry],
    pub visible_indices: Arc<Vec<usize>>,
    /// Position of the row with the longest message (sets the list's width).
    pub longest_row: usize,
    pub selected_row_pos: Option<usize>,
    /// The Highlight Rule paint of each entry (by entry position).
    pub paints: Arc<crate::highlight::RowPaints>,
    pub selection: Option<TextSelection>,
    /// Receives where the rows are on screen while painting.
    pub bounds_sink: std::rc::Rc<std::cell::Cell<gpui::Bounds<Pixels>>>,
    pub density: Density,
    /// Whether the minimap shows beside the table.
    pub show_minimap: bool,
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
                    // Spacer for the minimap and the scrollbar
                    .w(px(SCROLLBAR_WIDTH
                        + if state.show_minimap {
                            MINIMAP_WIDTH
                        } else {
                            0.0
                        }))
                    .child(""),
            )
            .child(anchored_header);

        // 2. Virtualized List using uniform_list
        // The view shows every entry, so an entry's position is its row.
        let longest_vis_pos = state.longest_row.min(total_visible.saturating_sub(1));
        let env = RowEnv {
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
            move |range, _window, cx| {
                // The rows are read from the view as each one is drawn, so a
                // frame never copies them.
                let mono_font = cx.theme().mono_font_family.clone();
                let this = env.view.read(cx);
                range
                    .filter_map(|vis_pos| env.row(vis_pos, &this.all_entries, &mono_font))
                    .collect::<Vec<_>>()
            },
        )
        .size_full()
        .with_width_from_item(Some(longest_vis_pos))
        .with_horizontal_sizing_behavior(ListHorizontalSizingBehavior::Unconstrained)
        .track_scroll(state.scroll_handle);

        // 3. Minimap: the whole log as one thin bar per row, with the part in view shaded.
        let colors = state.colors;
        let minimap_view = state.view.clone();
        let minimap_visible = state.visible_indices.clone();
        let minimap_matches = state.search_state.matched_shared.clone();
        let minimap_selected = state.selected_row_pos;
        let minimap_handle = state.scroll_handle.clone();
        let minimap_current = state.search_state.current_match_pos();
        let minimap_canvas = gpui::canvas(
            |_, _, _| (),
            move |bounds, _, window, cx| {
                let (start, rows_fit, first_visible, visible_rows) = minimap_window(
                    &minimap_handle,
                    minimap_visible.len(),
                    bounds.size.height,
                    row_height,
                );
                let entries = &minimap_view.read(cx).all_entries;
                let bar_max = MINIMAP_WIDTH - 10.0;
                for r in 0..rows_fit {
                    let pos = start + r;
                    let Some(entry) = minimap_visible.get(pos).and_then(|&ix| entries.get(ix))
                    else {
                        break;
                    };
                    let y = bounds.origin.y + px(r as f32 * MINIMAP_ROW_PX);
                    let is_match = minimap_matches.binary_search(&pos).is_ok();
                    if is_match || minimap_selected == Some(pos) {
                        let tint = if minimap_selected == Some(pos) {
                            colors.selected
                        } else if minimap_current == Some(pos) {
                            colors.search.opacity(0.6)
                        } else {
                            colors.search.opacity(0.3)
                        };
                        window.paint_quad(gpui::fill(
                            gpui::Bounds::new(
                                gpui::point(bounds.origin.x, y),
                                gpui::size(bounds.size.width, px(MINIMAP_ROW_PX)),
                            ),
                            tint,
                        ));
                    }
                    let color = if let Some(marker) = entry.markers.first() {
                        gpui::rgb(marker.color.rgb(colors.tone())).into()
                    } else if is_match {
                        colors.search
                    } else {
                        match entry.level {
                            LogLevel::Fatal | LogLevel::Error | LogLevel::Warn => {
                                colors.level(entry.level)
                            }
                            _ => colors.muted.opacity(0.5),
                        }
                    };
                    let width = (entry.payload.len() as f32 / 160.0).clamp(0.0, 1.0) * bar_max;
                    window.paint_quad(gpui::fill(
                        gpui::Bounds::new(
                            gpui::point(bounds.origin.x + px(4.0), y),
                            gpui::size(px(width.max(3.0)), px(MINIMAP_ROW_PX - 1.0)),
                        ),
                        color,
                    ));
                }

                let top = ((first_visible - start as f32) * MINIMAP_ROW_PX).max(0.0);
                let bottom = ((first_visible + visible_rows - start as f32) * MINIMAP_ROW_PX)
                    .min(f32::from(bounds.size.height));
                if bottom > top {
                    window.paint_quad(gpui::quad(
                        gpui::Bounds::new(
                            gpui::point(bounds.origin.x, bounds.origin.y + px(top)),
                            gpui::size(bounds.size.width, px((bottom - top).max(6.0))),
                        ),
                        px(0.0),
                        colors.muted.opacity(0.2),
                        px(1.0),
                        colors.muted.opacity(0.35),
                        gpui::BorderStyle::default(),
                    ));
                }
            },
        )
        .absolute()
        .size_full();

        // Scroll so the row under the mouse is centred in the list.
        fn scroll_to_minimap_y(
            handle: &UniformListScrollHandle,
            y: Pixels,
            total: usize,
            row_height: f32,
            window: &mut Window,
        ) {
            let base = handle.0.borrow().base_handle.clone();
            let list = base.bounds();
            if list.size.height <= px(0.0) || total == 0 {
                return;
            }
            let (start, _, _, _) = minimap_window(handle, total, list.size.height, row_height);
            let row = start as f32 + f32::from(y - list.origin.y) / MINIMAP_ROW_PX;
            let viewport = list.size.height;
            let max = base.max_offset().y.max(px(0.0));
            let top = (px(row * row_height) - viewport * 0.5).clamp(px(0.0), max);
            base.set_offset(gpui::point(base.offset().x, -top));
            window.refresh();
        }

        let (handle_down, handle_move) = (state.scroll_handle.clone(), state.scroll_handle.clone());
        let minimap_total = total_visible;
        let minimap = div()
            .id("minimap")
            .cursor_pointer()
            .flex_shrink_0()
            .w(px(MINIMAP_WIDTH))
            .h_full()
            .border_l_1()
            .relative()
            .child(minimap_canvas)
            .on_mouse_move(move |event: &gpui::MouseMoveEvent, window, _cx| {
                if event.pressed_button == Some(MouseButton::Left) {
                    scroll_to_minimap_y(
                        &handle_move,
                        event.position.y,
                        minimap_total,
                        row_height,
                        window,
                    );
                }
            })
            .on_mouse_down(
                MouseButton::Left,
                move |event: &gpui::MouseDownEvent, window, _cx| {
                    scroll_to_minimap_y(
                        &handle_down,
                        event.position.y,
                        minimap_total,
                        row_height,
                        window,
                    );
                },
            );

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
                        let alt_handle = state.scroll_handle.clone();
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
                            .min_w(px(0.0))
                            .h_full()
                            .overflow_hidden()
                            .relative()
                            .horizontal_scrollbar(state.scroll_handle)
                            .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                                v_down.update(cx, |this, cx| {
                                    this.start_text_selection(event.position, window, cx)
                                });
                            })
                            .on_mouse_down(MouseButton::Right, move |event, window, cx| {
                                v_right.update(cx, |this, cx| {
                                    this.on_cell_right_click(event.position, window, cx)
                                });
                            })
                            .on_mouse_move(move |event, window, cx| {
                                if event.pressed_button == Some(MouseButton::Left) {
                                    v_move.update(cx, |this, cx| {
                                        this.drag_text_selection(event.position, window, cx)
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
                                    move |bounds, _, window, _| {
                                        bounds_sink.set(bounds);
                                        boost_scroll_with_alt(&alt_handle, bounds, window);
                                    },
                                )
                                .absolute()
                                .size_full(),
                            )
                            .child(list)
                            .context_menu(move |menu, _window, cx| {
                                crate::ui::cell_menu::cell_menu(menu, &view_menu, cx)
                            })
                    })
                    .when(state.show_minimap, |el| el.child(minimap))
                    // The scrollbar sits at the far right, with the minimap to its left.
                    // Its own column, so it never covers the rows or the minimap.
                    .child(
                        div()
                            .w(px(SCROLLBAR_WIDTH))
                            .h_full()
                            .flex_shrink_0()
                            .relative()
                            .child(
                                div().absolute().inset_0().child(
                                    gpui_component::scroll::Scrollbar::new(state.scroll_handle)
                                        .id("canvas-vscrollbar")
                                        .axis(gpui_component::scroll::ScrollbarAxis::Vertical)
                                        .viewport_from_layout()
                                        .styles(|styles| styles.thumb(|t| t.min_length(px(120.0)))),
                                ),
                            ),
                    ),
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
    visible: Arc<Vec<usize>>,
    view: gpui::Entity<crate::app::AppView>,
    colors: CanvasColors,
    selected_pos: Option<usize>,
    search_matcher: Option<crate::search_expr::Matcher>,
    matched_positions: Arc<Vec<usize>>,
    current_match_pos: Option<usize>,
    scroll_handle: UniformListScrollHandle,
    layout: ColumnLayoutState,
    paints: Arc<crate::highlight::RowPaints>,
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

    fn row(
        &self,
        vis_pos: usize,
        entries: &[LogEntry],
        mono_font: &gpui::SharedString,
    ) -> Option<gpui::Stateful<gpui::Div>> {
        let colors = self.colors;
        let &entry_ix = self.visible.get(vis_pos)?;
        let entry = entries.get(entry_ix)?;
        let paint = self.paints.get(entry_ix).and_then(|p| p.as_deref());

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
            .font_family(mono_font.clone())
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
                            // Gutter: Line Number, in the stderr color for a line
                            // that came out of a command's standard error.
                            .child({
                                let stderr = entry.stream == Some(crate::model::Stream::Stderr);
                                div()
                                    .id(ElementId::from(("line-number", vis_pos)))
                                    .flex_shrink_0()
                                    .w(px(64.0))
                                    .px_2()
                                    .text_right()
                                    .when(stderr, |d| {
                                        d.text_color(colors.stderr).tooltip(|window, cx| {
                                            gpui_component::tooltip::Tooltip::new("stderr").build(window, cx)
                                        })
                                    })
                                    .child(crate::live::line_number_text(entry.index))
                            })
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
        let break_label = entry.break_before.as_ref().map(|mark| mark.label.clone());
        let break_actions = entry.break_before.as_ref().and_then(|mark| {
            let file_id = entry.file_id.as_deref()?.to_string();
            break_has_actions(mark.kind).then(|| BreakActions {
                view: self.view.clone(),
                file_id,
                kind: mark.kind,
                id: vis_pos,
            })
        });
        let label_left = scroll_x + anchored_width + px(8.0);
        Some(
            row_element
                .when_some(outline, |row, color| {
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
                })
                .when_some(break_label, |row, label| {
                    row.child(render_break(label, label_left, &colors, break_actions))
                }),
        )
    }
}

/// What a Break of a Remote File lets the user do about the lines above it.
pub(crate) struct BreakActions {
    pub view: gpui::Entity<crate::app::AppView>,
    pub file_id: String,
    pub kind: crate::live::BreakKind,
    /// Tells the buttons of two Breaks apart.
    pub id: usize,
}

/// Whether a Break of this kind sits where lines above are still on the
/// Server.
fn break_has_actions(kind: crate::live::BreakKind) -> bool {
    use crate::live::BreakKind;
    matches!(
        kind,
        BreakKind::LeftWindow
            | BreakKind::Cleared
            | BreakKind::StartOfLoaded
            | BreakKind::EarlierAvailable
            | BreakKind::EarlierWindowFull
    )
}

/// A small text button on a Break.
fn break_button(
    id: ElementId,
    label: &'static str,
    colors: &CanvasColors,
    on_click: impl Fn(&mut gpui::App) + 'static,
) -> gpui::AnyElement {
    div()
        .id(id)
        .cursor_pointer()
        .px_1()
        .rounded_sm()
        .border_1()
        .border_color(colors.warning.opacity(0.7))
        .hover(|s| s.bg(colors.warning.opacity(0.2)))
        .child(label)
        .on_click(move |_event, _window, cx| {
            cx.stop_propagation();
            on_click(cx);
        })
        .into_any_element()
}

/// A Break drawn on the line between two rows: a rule with a label. It is
/// not a row, so it takes no height of its own and sits half over the row
/// above.
pub(crate) fn render_break(
    label: String,
    label_left: Pixels,
    colors: &CanvasColors,
    actions: Option<BreakActions>,
) -> gpui::AnyElement {
    let buttons: Vec<gpui::AnyElement> = actions
        .map(|a| {
            use crate::live::BreakKind;
            if matches!(a.kind, BreakKind::EarlierAvailable | BreakKind::EarlierWindowFull) {
                let (view, file) = (a.view.clone(), a.file_id.clone());
                let full = a.kind == BreakKind::EarlierWindowFull;
                return vec![break_button(
                    ElementId::from(gpui::SharedString::from(format!("break-earlier-{}-{}", a.file_id, a.id))),
                    if full { "Open in full…" } else { "Load" },
                    colors,
                    move |cx| {
                        let id = file.clone();
                        view.update(cx, |this, cx| {
                            if full {
                                this.request_open_in_full(&id, cx);
                            } else {
                                this.load_earlier(&id, cx);
                            }
                        });
                    },
                )];
            }
            let more_view = a.view.clone();
            let more_file = a.file_id.clone();
            let mut buttons = vec![break_button(
                ElementId::from(gpui::SharedString::from(format!("break-more-{}-{}", a.file_id, a.id))),
                "Load more",
                colors,
                move |cx| {
                    let id = more_file.clone();
                    more_view.update(cx, |this, cx| this.load_more(&id, None, cx));
                },
            )];
            if a.kind == crate::live::BreakKind::StartOfLoaded {
                let (view, file) = (a.view.clone(), a.file_id.clone());
                buttons.push(break_button(
                    ElementId::from(gpui::SharedString::from(format!("break-back-{}-{}", a.file_id, a.id))),
                    "Back to Live Window",
                    colors,
                    move |cx| {
                        let id = file.clone();
                        view.update(cx, |this, cx| this.back_to_live_window(&id, cx));
                    },
                ));
            }
            buttons
        })
        .unwrap_or_default();
    div()
        .absolute()
        .top(px(-7.0))
        .left_0()
        .right_0()
        .h(px(14.0))
        .child(
            div()
                .absolute()
                .top(px(6.5))
                .left_0()
                .right_0()
                .h(px(1.0))
                .bg(colors.warning.opacity(0.7)),
        )
        .child(
            div()
                .absolute()
                .top_0()
                .left(label_left)
                .h(px(14.0))
                .px_2()
                .flex()
                .items_center()
                .rounded_sm()
                .border_1()
                .border_color(colors.warning.opacity(0.7))
                .bg(colors.header_bg)
                .text_color(colors.warning)
                .text_size(px(10.0))
                .whitespace_nowrap()
                .gap_2()
                .child(label)
                .children(buttons),
        )
        .into_any_element()
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

fn render_stream_cell(ctx: &CellCtx, width: Pixels) -> impl IntoElement {
    let text = ctx.entry.stream.map_or("", |s| s.as_str());
    let mut cell = div().flex_shrink_0().w(width).px_2().text_color(ctx.colors.muted).truncate();
    if let (Some(p), Some(bg)) = (ctx.paint, ctx.rule_cell) {
        if p.column == "stream" {
            cell = cell.bg(bg);
        }
    }
    cell.child(text.to_string())
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
        PinnableColumn::Stream => render_stream_cell(ctx, width).into_any_element(),
    }
}

/// How many times faster the wheel scrolls while Alt is held.
const ALT_SCROLL_FACTOR: f32 = 5.0;

/// While Alt is held, make wheel scrolling over `bounds` faster.
///
/// Runs in the capture phase, before the list scrolls by the plain wheel delta, and adds
/// the rest of the boost, so the list ends up moving `ALT_SCROLL_FACTOR` times as far.
/// Call it from the paint step of an element covering the list.
pub fn boost_scroll_with_alt(
    handle: &UniformListScrollHandle,
    bounds: gpui::Bounds<Pixels>,
    window: &mut Window,
) {
    let handle = handle.clone();
    let line_height = window.line_height();
    window.on_mouse_event(move |event: &gpui::ScrollWheelEvent, phase, window, _cx| {
        if phase != gpui::DispatchPhase::Capture
            || !event.modifiers.alt
            || !bounds.contains(&event.position)
        {
            return;
        }
        let base = handle.0.borrow().base_handle.clone();
        let extra = event.delta.pixel_delta(line_height) * (ALT_SCROLL_FACTOR - 1.0);
        let max = base.max_offset();
        let offset = base.offset();
        base.set_offset(gpui::point(
            (offset.x + extra.x).clamp(-max.x.max(px(0.0)), px(0.0)),
            (offset.y + extra.y).clamp(-max.y.max(px(0.0)), px(0.0)),
        ));
        window.refresh();
    });
}

/// Width of the column the vertical scrollbar sits in, at the far right.
const SCROLLBAR_WIDTH: f32 = 12.0;
/// Width of the minimap, left of the scrollbar.
const MINIMAP_WIDTH: f32 = 56.0;
/// Height of one row in the minimap.
const MINIMAP_ROW_PX: f32 = 3.0;

/// Which rows the minimap shows and where the list's viewport sits among them.
///
/// A log that fits is shown whole; a longer one slides along with the scroll position,
/// as in a text editor. Returns the first row shown, how many fit, the first row in view
/// and how many rows are in view.
fn minimap_window(
    handle: &UniformListScrollHandle,
    total: usize,
    height: Pixels,
    row_height: f32,
) -> (usize, usize, f32, f32) {
    let base = handle.0.borrow().base_handle.clone();
    let viewport = base.bounds().size.height;
    let max = base.max_offset().y.max(px(0.0));
    let top = (-base.offset().y).max(px(0.0)).min(max);
    let ratio = if max > px(0.0) { top / max } else { 0.0 };
    let rows_fit = (f32::from(height) / MINIMAP_ROW_PX) as usize;
    let start = if total > rows_fit {
        ((total - rows_fit) as f32 * ratio) as usize
    } else {
        0
    };
    let row_height = row_height.max(1.0);
    (
        start,
        rows_fit,
        f32::from(top) / row_height,
        f32::from(viewport) / row_height,
    )
}
