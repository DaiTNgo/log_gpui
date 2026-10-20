//! The Focus Pane: the lines the open Focus Tab lists, under the view and both
//! sidebars. A strip of tabs and the Focus Find field on top, then one row per
//! stored line with its number, Level Marker, Source File name, Markers and
//! the paint of Search, Focus Find and (in the Parsed View) Highlight Rules.

use crate::app::AppView;
use crate::app::raw::RawSection;
use crate::density::Density;
use crate::focus::{FocusFind, FocusPaneNotice, FocusRow};
use crate::highlight::RowPaint;
use crate::marker::{TextMarker, level_marker};
use crate::model::{LogEntry, LogLevel};
use crate::palette::Weight;
use crate::search_expr::Matcher;
use crate::text_selection::TextSelection;
use crate::ui::canvas::{CanvasColors, CellCtx, render_text_cell};
use crate::ui::cells::TextCell;
use gpui::prelude::FluentBuilder as _;
use gpui::{
    Bounds, ElementId, InteractiveElement, IntoElement, ListHorizontalSizingBehavior, MouseButton,
    ParentElement, Pixels, StatefulInteractiveElement, Styled, UniformListScrollHandle, div, px,
    uniform_list,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::{Input, InputState};
use gpui_component::menu::ContextMenuExt as _;
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::tag::Tag;
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::assets::IconName;
use std::collections::HashMap;
use std::sync::Arc;

/// Width of the line number, the Level Marker and the Source File name.
const NUMBER_WIDTH: f32 = 64.0;
const LEVEL_WIDTH: f32 = 24.0;
const SOURCE_WIDTH: f32 = 120.0;
/// Space before the text of a row.
const TEXT_PADDING: f32 = 8.0;
pub const TAB_STRIP_HEIGHT: f32 = 32.0;
/// Height of the collapsed pane: its strip of tabs and the border above.
pub const COLLAPSED_HEIGHT: f32 = TAB_STRIP_HEIGHT + 1.0;

/// Where the pane reads the text of its rows from.
pub enum PaneSource {
    Parsed {
        paints: Arc<crate::highlight::RowPaints>,
    },
    Raw {
        sections: Arc<Vec<RawSection>>,
        /// The Markers of each file by line (1-based), by `file_id`.
        markers: Arc<HashMap<String, HashMap<usize, Vec<TextMarker>>>>,
    },
}

/// One tab of the strip.
pub struct PaneTab {
    pub label: String,
    pub active: bool,
}

/// Where the pane is drawn.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PaneMode {
    /// Under the main window, at a height of its own.
    Docked,
    /// Filling a Focus Window, with the Search field of the open tab on top.
    Window,
}

pub struct PaneState<'a> {
    pub view: gpui::Entity<AppView>,
    pub mode: PaneMode,
    /// The Search field shown in a Focus Window.
    pub search_input: Option<&'a gpui::Entity<InputState>>,
    pub height: f32,
    /// Collapsed, the pane is just its strip of tabs.
    pub expanded: bool,
    pub tabs: Vec<PaneTab>,
    pub can_close: bool,
    pub find_input: &'a gpui::Entity<InputState>,
    pub find: FocusFind,
    pub search_matcher: Option<Matcher>,
    pub rows: Arc<Vec<FocusRow>>,
    pub source: PaneSource,
    /// Index in `rows` of the current line, when the tab lists it.
    pub selected: Option<usize>,
    /// Source File names are shown (a Log Space of several files).
    pub show_source: bool,
    /// Text of the empty pane, when there is nothing to list.
    pub notice: Option<FocusPaneNotice>,
    pub density: Density,
    pub colors: CanvasColors,
    pub scroll_handle: &'a UniformListScrollHandle,
    pub selection: Option<TextSelection>,
    pub bounds_sink: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>>,
    /// Index in `rows` of the widest row. Computed by the caller: the pane
    /// renders inside the AppView's own render, so it must not read it back.
    pub longest: usize,
    /// In the Parsed View, the view's columns: the pane shows the same ones,
    /// in the same order and widths, under their Column Labels. `None` in the
    /// Raw View, where the pane shows each raw line.
    pub columns: Option<crate::column::ColumnLayoutState>,
}

/// Width taken by the cells before the text of a row.
pub fn text_offset(parsed: bool, show_source: bool) -> f32 {
    NUMBER_WIDTH
        + if parsed { LEVEL_WIDTH } else { 0.0 }
        + if show_source { SOURCE_WIDTH } else { 0.0 }
        + TEXT_PADDING
}

pub struct FocusPane;

impl FocusPane {
    pub fn render(state: PaneState, cx: &mut gpui::App) -> gpui::AnyElement {
        let height = state.height;
        let pane_bg = state.colors.background;
        let theme = cx.theme();
        let (border, muted, bg, fg) = (theme.border, theme.muted_foreground, theme.background, theme.foreground);
        let tab_active_bg = theme.list_active;

        let mut strip = h_flex().gap_1().items_center().min_w(px(0.0));
        let can_close = state.can_close;
        for (i, tab) in state.tabs.iter().enumerate() {
            let (v_open, v_close) = (state.view.clone(), state.view.clone());
            strip = strip.child(
                h_flex()
                    .id(ElementId::from(("focus-tab", i)))
                    .h(px(24.0))
                    .px_2()
                    .gap_1()
                    .items_center()
                    .rounded_md()
                    .cursor_pointer()
                    .text_xs()
                    .when(tab.active, |this| this.bg(tab_active_bg).font_weight(gpui::FontWeight::SEMIBOLD))
                    .when(!tab.active, |this| this.text_color(muted))
                    .child(tab.label.clone())
                    .when(can_close, |this| {
                        this.child(
                            div()
                                .id(ElementId::from(("focus-tab-close", i)))
                                .flex()
                                .items_center()
                                .rounded_sm()
                                .child(Icon::new(IconName::Close).size_3())
                                .on_click(move |_, _window, cx| {
                                    cx.stop_propagation();
                                    v_close.update(cx, |this, cx| this.close_focus_tab(i, cx));
                                }),
                        )
                    })
                    .on_click(move |_, _window, cx| {
                        v_open.update(cx, |this, cx| this.switch_focus_tab(i, cx));
                    }),
            );
        }
        let v_add = state.view.clone();
        strip = strip.child(
            Button::new("focus-tab-add")
                .ghost()
                .xsmall()
                .icon(IconName::Plus)
                .on_click(move |_, window, cx| {
                    v_add.update(cx, |this, cx| this.add_focus_tab(window, cx));
                }),
        );

        let find_label = state.find.label();
        let find_error = state.find.parse_error.is_some();
        let (v_prev, v_next, v_hide) = (state.view.clone(), state.view.clone(), state.view.clone());
        let v_detach = state.view.clone();
        let find_box = h_flex()
            .gap_1()
            .items_center()
            .flex_shrink_0()
            .child(
                h_flex()
                    .w(px(240.0))
                    .h(px(24.0))
                    .px_2()
                    .gap_1()
                    .items_center()
                    .rounded_md()
                    .border_1()
                    .border_color(if find_error { cx.theme().danger } else { border })
                    .bg(bg)
                    .child(Icon::new(IconName::Search).size_3().text_color(muted))
                    .child(
                        div().flex_1().min_w(px(0.0)).child(
                            Input::new(state.find_input)
                                .appearance(false)
                                .bordered(false)
                                .focus_bordered(false)
                                .h(px(22.0)),
                        ),
                    )
                    .when(find_error, |this| this.child(Tag::danger().child("!"))),
            )
            .child(
                Button::new("focus-find-prev")
                    .ghost()
                    .xsmall()
                    .icon(IconName::ChevronUp)
                    .on_click(move |_, _w, cx| v_prev.update(cx, |this, cx| this.focus_find_prev(cx))),
            )
            .child(
                Button::new("focus-find-next")
                    .ghost()
                    .xsmall()
                    .icon(IconName::ChevronDown)
                    .on_click(move |_, _w, cx| v_next.update(cx, |this, cx| this.focus_find_next(cx))),
            )
            .child(div().min_w(px(40.0)).text_xs().text_color(muted).child(find_label))
            .map(|this| match state.mode {
                PaneMode::Docked => this
                    .child(
                        Button::new("focus-pane-detach")
                            .ghost()
                            .xsmall()
                            .icon(IconName::ExternalLink)
                            .tooltip("Move to window")
                            .on_click(move |_, window, cx| {
                                v_detach.update(cx, |this, cx| this.move_focus_to_window(window, cx))
                            }),
                    )
                    .child(
                        Button::new("focus-pane-collapse")
                            .ghost()
                            .xsmall()
                            .icon(if state.expanded { IconName::ChevronDown } else { IconName::ChevronUp })
                            .on_click(move |_, _w, cx| v_hide.update(cx, |this, cx| this.toggle_focus_pane(cx))),
                    ),
                PaneMode::Window => this.child(
                    Button::new("focus-pane-back")
                        .ghost()
                        .xsmall()
                        .icon(IconName::PictureInPicture2)
                        .tooltip("Move back")
                        .on_click(move |_, window, cx| {
                            v_detach.update(cx, |this, cx| this.move_focus_back(window, cx))
                        }),
                ),
            });

        let header = h_flex()
            .w_full()
            .h(px(TAB_STRIP_HEIGHT))
            .px_2()
            .gap_2()
            .items_center()
            .flex_shrink_0()
            .border_b_1()
            .border_color(border)
            .when_some(state.search_input.filter(|_| state.mode == PaneMode::Window), |this, input| {
                this.child(
                    h_flex()
                        .w(px(280.0))
                        .h(px(24.0))
                        .px_2()
                        .gap_1()
                        .items_center()
                        .flex_shrink_0()
                        .rounded_md()
                        .border_1()
                        .border_color(border)
                        .bg(bg)
                        .child(Icon::new(IconName::Search).size_3().text_color(muted))
                        .child(
                            div().flex_1().min_w(px(0.0)).child(
                                Input::new(input).appearance(false).bordered(false).focus_bordered(false).h(px(22.0)),
                            ),
                        ),
                )
            })
            .child(div().flex_1().min_w(px(0.0)).overflow_hidden().child(strip))
            .child(find_box);

        let window_mode = state.mode == PaneMode::Window;
        let expanded = state.expanded || window_mode;
        let body = if !expanded {
            div().into_any_element()
        } else if state.rows.is_empty() {
            match &state.notice {
                Some(notice) => {
                    let v_clear = state.view.clone();
                    v_flex()
                        .flex_1()
                        .items_center()
                        .justify_center()
                        .gap_1()
                        .child(div().text_sm().text_color(if *notice == FocusPaneNotice::Hint {
                            muted
                        } else {
                            cx.theme().foreground
                        }).child(notice.title()))
                        .when_some(notice.detail(), |this, detail| {
                            this.child(div().text_xs().text_color(muted).child(detail))
                        })
                        .when_some(
                            match notice {
                                FocusPaneNotice::UnreadColumn { key, can_turn_on, .. } => Some((key.clone(), *can_turn_on)),
                                _ => None,
                            },
                            |this, (key, can_turn_on)| {
                                let (v_remove, v_on) = (state.view.clone(), state.view.clone());
                                let on_key = key.clone();
                                this.child(
                                    h_flex()
                                        .gap_2()
                                        .pt_1()
                                        .child(Button::new("focus-pane-remove-condition").small().label("Remove condition").on_click(
                                            move |_, _w, cx| v_remove.update(cx, |this, cx| this.filter_clear_column(&key, cx)),
                                        ))
                                        .when(can_turn_on, |d| {
                                            d.child(
                                                Button::new("focus-pane-turn-on").small().primary().label("Turn column back on").on_click(
                                                    move |_, _w, cx| v_on.update(cx, |this, cx| this.turn_column_back_on(&on_key, cx)),
                                                ),
                                            )
                                        }),
                                )
                            },
                        )
                        .when(notice.offers_clear_filter(), |this| {
                            this.child(
                                Button::new("focus-pane-clear-filter")
                                    .small()
                                    .label("Clear Filter")
                                    .on_click(move |_, _w, cx| {
                                        v_clear.update(cx, |this, cx| this.filter_clear_all(cx))
                                    }),
                            )
                        })
                        .into_any_element()
                }
                None => div().flex_1().into_any_element(),
            }
        } else {
            Self::render_rows(state, cx)
        };

        div()
            .id("focus-pane")
            .w_full()
            .map(|this| {
                if window_mode {
                    this.size_full()
                } else {
                    this.h(px(if expanded { height } else { COLLAPSED_HEIGHT })).flex_shrink_0().border_t_1()
                }
            })
            .flex()
            .flex_col()
            .bg(pane_bg)
            .text_color(fg)
            .border_color(border)
            .child(header)
            .child(body)
            .into_any_element()
    }

    fn render_rows(state: PaneState, cx: &mut gpui::App) -> gpui::AnyElement {
        // The widest row sets the width the list scrolls to. The rows are
        // read from the view, never copied.
        let longest = state.longest;
        let count = state.rows.len();
        let scroll_handle = state.scroll_handle.clone();
        let alt_handle = state.scroll_handle.clone();
        let bounds_sink = state.bounds_sink.clone();
        let view = state.view.clone();
        let menu_view = view.clone();
        let (v_down, v_move, v_up, v_up_out) = (view.clone(), view.clone(), view.clone(), view);
        let header = state.columns.as_ref().map(|layout| render_column_header(layout, &scroll_handle, &state.colors));
        let env = PaneRows {
            columns: state.columns.clone(),
            view: state.view.clone(),
            rows: state.rows,
            source: state.source,
            selected: state.selected,
            find: state.find,
            search_matcher: state.search_matcher,
            show_source: state.show_source,
            colors: state.colors,
            selection: state.selection,
            row_height: state.density.row_height(),
            font_size: state.density.font_size(),
        };
        let list = uniform_list("focus_pane_list", count, move |range, _window, cx| {
            let mono_font = cx.theme().mono_font_family.clone();
            let this = env.view.read(cx);
            range.filter_map(|i| env.row(i, &this.all_entries, &mono_font)).collect::<Vec<_>>()
        })
        .size_full()
        .with_width_from_item(Some(longest))
        .with_horizontal_sizing_behavior(ListHorizontalSizingBehavior::Unconstrained)
        .track_scroll(state.scroll_handle);

        let container = div()
            .id("focus_pane_container")
            .flex_1()
            .min_w(px(0.0))
            .h_full()
            .overflow_hidden()
            .relative()
            .horizontal_scrollbar(&scroll_handle)
            .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                v_down.update(cx, |this, cx| this.start_pane_selection(event.position, window, cx));
            })
            .on_mouse_move(move |event, window, cx| {
                if event.pressed_button == Some(MouseButton::Left) {
                    v_move.update(cx, |this, cx| this.drag_pane_selection(event.position, window, cx));
                }
            })
            .on_mouse_up(MouseButton::Left, move |event, window, cx| {
                v_up.update(cx, |this, cx| this.finish_pane_selection(event.position, window, cx));
            })
            .on_mouse_up_out(MouseButton::Left, move |event, window, cx| {
                v_up_out.update(cx, |this, cx| this.finish_pane_selection(event.position, window, cx));
            })
            .child(
                gpui::canvas(
                    |bounds, _, _| bounds,
                    move |bounds, _, window, _| {
                        bounds_sink.set(bounds);
                        crate::ui::canvas::boost_scroll_with_alt(&alt_handle, bounds, window);
                    },
                )
                    .absolute()
                    .size_full(),
            )
            .child(list)
            .context_menu(move |menu, _window, cx| crate::ui::cell_menu::pane_menu(menu, &menu_view, cx));

        // The vertical scrollbar has its own column at the far right, so it never covers the rows.
        let body = div()
            .flex_1()
            .min_h(px(0.0))
            .flex()
            .flex_row()
            .child(container)
            .child(
                div().w(px(12.0)).h_full().flex_shrink_0().relative().child(
                    div().absolute().inset_0().child(
                        gpui_component::scroll::Scrollbar::new(&scroll_handle)
                            .id("focus-pane-vscrollbar")
                            .axis(gpui_component::scroll::ScrollbarAxis::Vertical)
                            .viewport_from_layout()
                            .styles(|styles| styles.thumb(|t| t.min_length(px(120.0)))),
                    ),
                ),
            )
            .into_any_element();
        match header {
            Some(header) => gpui_component::v_flex()
                .flex_1()
                .min_h(px(0.0))
                .child(header)
                .child(body)
                .into_any_element(),
            None => body,
        }
    }
}

/// What the virtualized list needs to build the rows on its own.
struct PaneRows {
    /// The view's columns, in the Parsed View.
    columns: Option<crate::column::ColumnLayoutState>,
    view: gpui::Entity<AppView>,
    rows: Arc<Vec<FocusRow>>,
    source: PaneSource,
    selected: Option<usize>,
    find: FocusFind,
    search_matcher: Option<Matcher>,
    show_source: bool,
    colors: CanvasColors,
    selection: Option<TextSelection>,
    row_height: f32,
    font_size: f32,
}

impl PaneRows {
    fn row(
        &self,
        index: usize,
        entries: &[LogEntry],
        mono_font: &gpui::SharedString,
    ) -> Option<gpui::Stateful<gpui::Div>> {
        let row = *self.rows.get(index)?;
        let colors = self.colors;
        // Each row is read as a `LogEntry`: the Parsed View has them, a Raw
        // line gets a plain one that carries its Markers.
        let raw_entry;
        let break_label: Option<String>;
        let (entry, paint, number, source_name): (&LogEntry, Option<&RowPaint>, usize, Option<String>) =
            match (&self.source, row) {
                (PaneSource::Parsed { paints }, FocusRow::Entry(ix)) => {
                    let e = entries.get(ix)?;
                    break_label = e.break_before.as_ref().map(|m| m.label.clone());
                    (e, paints.get(ix).and_then(|p| p.as_deref()), e.index, e.source.as_deref().map(str::to_string))
                }
                (PaneSource::Raw { sections, markers }, FocusRow::Raw { section, line }) => {
                    let s = sections.get(section)?;
                    let text = s.lines.get(line)?;
                    let mut e = LogEntry::new(
                        line + 1,
                        0.0,
                        String::new(),
                        String::new(),
                        LogLevel::Unknown,
                        text.clone(),
                        text.clone(),
                    );
                    e.markers = markers
                        .get(&s.file_id)
                        .and_then(|m| m.get(&(line + 1)))
                        .cloned()
                        .unwrap_or_default();
                    raw_entry = e;
                    break_label = s.breaks.get(&line).map(|m| m.label.clone());
                    (&raw_entry, None, line + 1, Some(s.display_name.clone()))
                }
                _ => return None,
            };
        let parsed = matches!(self.source, PaneSource::Parsed { .. });
        let tone = colors.tone();
        let base_bg = match paint {
            Some(p) => colors
                .background
                .blend(gpui::rgb(p.color.with_weight(Weight::Row, tone)).into()),
            None => colors.background,
        };
        let is_selected = self.selected == Some(index);
        let is_find_current = self.find.current_row() == Some(index);
        let search_ranges = match &self.search_matcher {
            Some(m) if m.is_match(&entry.raw_line) => m.find_ranges(&entry.raw_line),
            _ => Vec::new(),
        };
        let find_ranges = self.find.find_ranges(&entry.raw_line);
        let selected_in_line = self.selection.and_then(|s| s.range_in_row(index, entry.raw_line.len()));
        let columns = self.columns.as_ref().filter(|_| parsed);
        let hidden_match = columns.and_then(|layout| {
            crate::ui::cells::hidden_matches(entry, &search_ranges, layout)
                .map(|hidden| crate::ui::cells::hidden_matches_tooltip(&hidden, layout))
        });
        // In columns, Search is painted on whichever cell holds each Match.
        let (cell_search, line_search): (&[(usize, usize)], &[(usize, usize)]) =
            if columns.is_some() { (&[], &search_ranges) } else { (&search_ranges, &[]) };
        let ctx = CellCtx {
            colors: &colors,
            entry,
            paint,
            search_ranges: cell_search,
            find_ranges: &find_ranges,
            line_search,
            selected: selected_in_line.as_ref(),
            tone,
            rule_cell: None,
        };
        let level_symbol = level_marker(entry.level).map_or(" ", |m| m.symbol());
        // The current Match outranks the selected row, as on the Log Canvas.
        let outline = if is_find_current {
            Some(colors.find)
        } else if is_selected {
            Some(colors.header_fg)
        } else {
            None
        };
        Some(
            div()
                .id(ElementId::from(("focus-row", index)))
                .h(px(self.row_height))
                .min_w_full()
                .relative()
                .flex()
                .flex_row()
                .items_center()
                // Monospace, like the Log Canvas rows: hit-testing a pointer
                // assumes every character is the same width.
                .font_family(mono_font.clone())
                .text_size(px(self.font_size))
                .bg(if is_selected { base_bg.blend(colors.selected.opacity(0.5)) } else { base_bg })
                .child({
                    // Muted, or the stderr color for a line of a command's
                    // standard error.
                    let stderr = entry.stream == Some(crate::model::Stream::Stderr);
                    div()
                        .id(ElementId::from(("focus-line-number", index)))
                        .flex_shrink_0()
                        .w(px(NUMBER_WIDTH))
                        .px_2()
                        .text_right()
                        .text_color(if stderr { colors.stderr } else { colors.muted })
                        .when(stderr, |d| {
                            d.tooltip(|window, cx| gpui_component::tooltip::Tooltip::new("stderr").build(window, cx))
                        })
                        .relative()
                        .when_some(hidden_match, |d, tip| {
                            d.child(crate::ui::canvas::hidden_match_mark(
                                ElementId::from(("focus-hidden-match", index)),
                                tip,
                                &colors,
                            ))
                        })
                        .child(crate::live::line_number_text(number))
                })
                .when(parsed, |this| {
                    this.child(
                        div()
                            .flex_shrink_0()
                            .w(px(LEVEL_WIDTH))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_xs()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(colors.level(entry.level))
                            .child(level_symbol),
                    )
                })
                .when_some(columns, |this, layout| {
                    let mut this = this;
                    for col in layout.columns() {
                        this = this.child(crate::ui::canvas::render_column_cell(col, layout, &ctx));
                    }
                    this.child(
                        div()
                            .flex_1()
                            .px_2()
                            .whitespace_nowrap()
                            .pr_16()
                            .child(render_text_cell(&ctx, TextCell::Message)),
                    )
                })
                .when(columns.is_none() && self.show_source, |this| {
                    this.child(
                        div()
                            .flex_shrink_0()
                            .w(px(SOURCE_WIDTH))
                            .px_2()
                            .text_color(colors.muted)
                            .truncate()
                            .child(source_name.unwrap_or_default()),
                    )
                })
                .when(columns.is_none(), |this| {
                    this.child(
                        div()
                            .flex_1()
                            .px_2()
                            .whitespace_nowrap()
                            .pr_16()
                            .child(render_text_cell(&ctx, TextCell::Raw)),
                    )
                })
                .when_some(outline, |row, color| {
                    row.child(
                        div().absolute().top_0().left_0().right_0().bottom_0().border_1().border_color(color),
                    )
                })
                .when_some(break_label, |row, label| {
                    row.child(crate::ui::canvas::render_break(label, px(NUMBER_WIDTH + 8.0), &colors, None, false))
                }),
        )
    }
}

/// The Column Headers of the pane in the Parsed View: the view's columns
/// under their Column Labels, sliding with the pane's horizontal scroll.
fn render_column_header(
    layout: &crate::column::ColumnLayoutState,
    scroll_handle: &gpui::UniformListScrollHandle,
    colors: &CanvasColors,
) -> gpui::AnyElement {
    let scroll_x = crate::ui::canvas::horizontal_scroll_x(scroll_handle);
    let mut row = h_flex()
        .relative()
        .left(-scroll_x)
        .h_full()
        .items_center()
        .child(div().flex_shrink_0().w(px(NUMBER_WIDTH + LEVEL_WIDTH)));
    for col in layout.columns() {
        row = row.child(
            div()
                .flex_shrink_0()
                .w(layout.width(col))
                .when(col.is_compact(), |d| d.px_1())
                .when(!col.is_compact(), |d| d.px_2())
                .truncate()
                .child(layout.header_text(col).to_string()),
        );
    }
    let message = layout.labels.get("message").cloned().unwrap_or_else(|| "MESSAGE".to_string());
    row = row.child(div().px_2().whitespace_nowrap().child(message));
    div()
        .id("focus-pane-column-header")
        .h(px(22.0))
        .w_full()
        .flex_shrink_0()
        .overflow_hidden()
        .border_b_1()
        .border_color(colors.border)
        .bg(colors.header_bg)
        .text_color(colors.header_fg)
        .text_xs()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .child(row)
        .into_any_element()
}
