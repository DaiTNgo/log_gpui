//! The Time Histogram strip (openspec change `add-time-histogram`, decision
//! 4): one column per span of time, painted with `gpui::canvas` like the
//! minimap. From the bottom up: the Time Range's background, the columns
//! (all lines dim under a Filter, the lines passing it solid), the ticks of
//! the columns with a Match, and the frame of the part the list shows.
//! Under the columns, on the same axis, the Swimlanes (openspec change
//! `add-swimlane`, decision 6): a row of bars per value, with the names in
//! a gutter on the left that the columns line up with.

use crate::app::AppView;
use crate::app::time_histogram::HistogramDrag;
use crate::app::swimlane::LaneChoice;
use crate::time_histogram::{Buckets, Grid, Lane, Lanes, TimeAxis};
use crate::ui::canvas::CanvasColors;
use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, Bounds, Entity, InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels, StatefulInteractiveElement,
    Styled, Window, div, px,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_component::Sizable as _;
use std::cell::Cell;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

/// Height of the columns, open and folded.
const BARS_HEIGHT: f32 = 64.0;
const FOLDED_HEIGHT: f32 = 24.0;
/// Height of the label row above the columns.
const LABELS_HEIGHT: f32 = 18.0;
/// Room under the columns for the Match ticks.
const TICK_HEIGHT: f32 = 3.0;
/// How near an edge of the Time Range a press takes that edge, in pixels.
const EDGE_PX: f32 = 4.0;
/// Height of a Swimlane, and of its bar in it.
const LANE_HEIGHT: f32 = 14.0;
const LANE_BAR: f32 = 8.0;
/// Width of the lane names on the left; the columns start after it.
const NAMES_WIDTH: f32 = 90.0;
/// A name longer than this may be cut: it has a tooltip.
const NAME_CHARS: usize = 14;

/// The "Lane by" menu.
pub struct LaneMenu {
    /// The column the lanes split by now, as its label.
    pub current: Option<(String, String)>,
    pub choices: Vec<LaneChoice>,
    /// The key of the column suggested, when the view has no lanes.
    pub suggested: Option<String>,
}

/// What one histogram strip shows.
pub struct HistogramProps {
    pub view: Entity<AppView>,
    pub axis: TimeAxis,
    pub buckets: Arc<Buckets>,
    /// The Time Range, as times (an open end is `None`).
    pub time_range: Option<(Option<f64>, Option<f64>)>,
    /// The Time Range a drag under way would set.
    pub drag_range: Option<(f64, f64)>,
    /// While files are read: the time read so far; the axis after it is dim.
    pub indexed_to: Option<f64>,
    pub colors: CanvasColors,
    pub collapsed: bool,
    /// Receives where the columns are drawn.
    pub bounds_sink: Rc<Cell<Bounds<Pixels>>>,
    /// The Swimlanes drawn, and their names.
    pub lanes: Vec<(Lane, String)>,
    pub lane_menu: LaneMenu,
}

/// Where time `t` is across `bounds` showing `visible`.
fn x_of(t: f64, visible: &Range<f64>, bounds: &Bounds<Pixels>) -> f32 {
    let span = (visible.end - visible.start).max(1e-9);
    f32::from(bounds.origin.x) + ((t - visible.start) / span) as f32 * f32::from(bounds.size.width)
}

/// The time at `x` (window coordinates) across `bounds` showing `visible`.
pub fn time_at(x: Pixels, visible: &Range<f64>, bounds: &Bounds<Pixels>) -> f64 {
    let w = f32::from(bounds.size.width).max(1.0);
    let ratio = f64::from((f32::from(x) - f32::from(bounds.origin.x)) / w);
    visible.start + ratio.clamp(0.0, 1.0) * (visible.end - visible.start)
}

/// A time on the axis's labels: the time of day, with the date when the
/// axis spans more than a day.
fn label(t: f64, span: f64) -> String {
    let Some(dt) = chrono::DateTime::from_timestamp(t.floor() as i64, 0).map(|d| d.naive_utc()) else {
        return String::new();
    };
    if span > 86_400.0 { dt.format("%m-%d %H:%M").to_string() } else { dt.format("%H:%M:%S").to_string() }
}

fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Bounds<Pixels> {
    Bounds::new(gpui::point(px(x0), px(y0)), gpui::size(px((x1 - x0).max(0.0)), px((y1 - y0).max(0.0))))
}

/// Paint the columns of `buckets` on `grid` between `top` and `bottom`.
fn paint_columns(
    window: &mut Window,
    buckets: &Buckets,
    grid: &Grid,
    visible: &Range<f64>,
    bounds: &Bounds<Pixels>,
    (top, bottom): (f32, f32),
    colors: &CanvasColors,
) {
    let max = buckets.max_total().max(1) as f32;
    let height = bottom - top;
    let fills = [colors.muted.opacity(0.55), colors.warning, colors.danger];
    // Bottom up: errors, then warnings, then the rest.
    let stack = |window: &mut Window, x0: f32, x1: f32, counts: &[u32; 3], opacity: f32| {
        let mut y = bottom;
        for class in [2, 1, 0] {
            let h = counts[class] as f32 / max * height;
            if h <= 0.0 {
                continue;
            }
            // A column with a line is never thinner than a pixel.
            let h = h.max(1.0);
            window.paint_quad(gpui::fill(rect(x0, y - h, x1, y), fills[class].opacity(fills[class].a * opacity)));
            y -= h;
        }
    };
    for (i, counts) in buckets.counts.iter().enumerate() {
        if counts.iter().all(|&c| c == 0) {
            continue;
        }
        let x0 = x_of(grid.start_of(i), visible, bounds).max(f32::from(bounds.origin.x));
        let x1 = x_of(grid.start_of(i + 1), visible, bounds).min(f32::from(bounds.right()));
        let x1 = if x1 - x0 > 3.0 { x1 - 1.0 } else { x1 };
        match &buckets.filtered {
            Some(filtered) => {
                stack(window, x0, x1, counts, 0.3);
                stack(window, x0, x1, &filtered[i], 1.0);
            }
            None => stack(window, x0, x1, counts, 1.0),
        }
    }
    for (i, _) in buckets.matched.iter().enumerate().filter(|(_, m)| **m) {
        let x0 = x_of(grid.start_of(i), visible, bounds).max(f32::from(bounds.origin.x));
        let x1 = x_of(grid.start_of(i + 1), visible, bounds).min(f32::from(bounds.right()));
        window.paint_quad(gpui::fill(rect(x0, bottom + 1.0, x1.max(x0 + 2.0), bottom + TICK_HEIGHT), colors.search));
    }
}

/// Paint the bars of each lane of `order`, one row of `LANE_HEIGHT` each
/// from `top` down: a bar where the lane has a Parent Line in a column,
/// coloured by the highest level there.
#[allow(clippy::too_many_arguments)]
fn paint_lanes(
    window: &mut Window,
    lanes: &Lanes,
    order: &[Lane],
    grid: &Grid,
    visible: &Range<f64>,
    bounds: &Bounds<Pixels>,
    top: f32,
    colors: &CanvasColors,
) {
    let fills = [colors.muted.opacity(0.55), colors.warning, colors.danger];
    for (i, lane) in order.iter().enumerate() {
        let y0 = top + i as f32 * LANE_HEIGHT;
        window.paint_quad(gpui::fill(rect(f32::from(bounds.origin.x), y0, f32::from(bounds.right()), y0 + 0.5), colors.border.opacity(0.5)));
        let bar = y0 + (LANE_HEIGHT - LANE_BAR) / 2.0;
        for (col, &level) in lanes.level(*lane).iter().enumerate() {
            if level == 0 {
                continue;
            }
            let x0 = x_of(grid.start_of(col), visible, bounds).max(f32::from(bounds.origin.x));
            let x1 = x_of(grid.start_of(col + 1), visible, bounds).min(f32::from(bounds.right()));
            let x1 = if x1 - x0 > 3.0 { x1 - 1.0 } else { x1.max(x0 + 1.0) };
            window.paint_quad(gpui::fill(rect(x0, bar, x1, bar + LANE_BAR), fills[usize::from(level - 1).min(2)]));
        }
    }
}

/// The Time Range's background, with a handle at each closed edge.
fn paint_range(window: &mut Window, range: (Option<f64>, Option<f64>), visible: &Range<f64>, bounds: &Bounds<Pixels>, colors: &CanvasColors, strong: bool) {
    let (top, bottom) = (f32::from(bounds.origin.y), f32::from(bounds.bottom()));
    let left = range.0.map_or(f32::from(bounds.origin.x), |t| x_of(t, visible, bounds));
    let right = range.1.map_or(f32::from(bounds.right()), |t| x_of(t, visible, bounds));
    let (left, right) = (left.max(f32::from(bounds.origin.x)), right.min(f32::from(bounds.right())));
    if right <= left {
        return;
    }
    window.paint_quad(gpui::fill(rect(left, top, right, bottom), colors.info.opacity(if strong { 0.22 } else { 0.14 })));
    for (edge, x) in [(range.0, left), (range.1, right)] {
        if edge.is_some() {
            window.paint_quad(gpui::fill(rect(x - 1.5, top, x + 1.5, bottom), colors.info.opacity(0.7)));
        }
    }
}

pub fn render(props: HistogramProps, cx: &App) -> gpui::AnyElement {
    let HistogramProps {
        view,
        axis,
        buckets,
        time_range,
        drag_range,
        indexed_to,
        colors,
        collapsed,
        bounds_sink,
        lanes,
        lane_menu,
    } = props;
    let visible = axis.visible.clone();
    let grid = buckets.grid.unwrap_or_else(|| axis.grid());
    let bars = if collapsed { FOLDED_HEIGHT } else { BARS_HEIGHT };
    // A folded strip has no lanes.
    let lanes = if collapsed || buckets.lanes.is_none() { Vec::new() } else { lanes };
    let order: Rc<Vec<Lane>> = Rc::new(lanes.iter().map(|(l, _)| *l).collect());
    let height = bars + order.len() as f32 * LANE_HEIGHT;

    let paint_view = view.clone();
    let sink = bounds_sink.clone();
    let paint_visible = visible.clone();
    let paint_colors = colors.clone();
    let paint_buckets = buckets.clone();
    let paint_order = order.clone();
    let canvas = gpui::canvas(
        |_, _, _| (),
        move |bounds, _, window, cx| {
            let width_changed = sink.get().size.width != bounds.size.width;
            sink.set(bounds);
            if width_changed {
                // The step follows the width: count again for it.
                let view = paint_view.clone();
                cx.defer(move |cx| view.update(cx, |this, cx| this.refresh_histogram(cx)));
            }
            let colors = &paint_colors;
            let visible = &paint_visible;
            let bars_bottom = f32::from(bounds.origin.y) + bars;
            let (top, bottom) = (f32::from(bounds.origin.y) + 2.0, bars_bottom - TICK_HEIGHT - 1.0);
            if let Some(range) = drag_range.map(|(a, b)| (Some(a), Some(b))).or(time_range) {
                paint_range(window, range, visible, &bounds, colors, drag_range.is_some());
            }
            paint_columns(window, &paint_buckets, &grid, visible, &bounds, (top, bottom), colors);
            if let Some(lanes) = paint_buckets.lanes.as_ref().filter(|_| !paint_order.is_empty()) {
                paint_lanes(window, lanes, &paint_order, &grid, visible, &bounds, bars_bottom, colors);
            }
            if let Some(read_to) = indexed_to.filter(|t| *t < visible.end) {
                // Not counted yet: dim.
                let x = x_of(read_to, visible, &bounds).max(f32::from(bounds.origin.x));
                window.paint_quad(gpui::fill(
                    rect(x, f32::from(bounds.origin.y), f32::from(bounds.right()), f32::from(bounds.bottom())),
                    colors.background.opacity(0.6),
                ));
                window.paint_quad(gpui::fill(rect(x, bottom - 1.0, f32::from(bounds.right()), bottom), colors.muted.opacity(0.4)));
            }
            if let Some(sight) = paint_view.read(cx).histogram_in_sight() {
                let x0 = x_of(sight.start, visible, &bounds).max(f32::from(bounds.origin.x));
                let x1 = x_of(sight.end, visible, &bounds).min(f32::from(bounds.right())).max(x0 + 2.0);
                window.paint_quad(gpui::quad(
                    rect(x0, f32::from(bounds.origin.y), x1, f32::from(bounds.bottom())),
                    px(2.0),
                    gpui::transparent_black(),
                    px(1.0),
                    colors.header_fg.opacity(0.6),
                    gpui::BorderStyle::default(),
                ));
            }
        },
    )
    .absolute()
    .size_full();

    let mut columns = div().id("time_histogram").relative().flex_1().min_w_0().h(px(height)).child(canvas);
    // ⌘ + scroll zooms around the pointer.
    {
        let view = view.clone();
        let sink = bounds_sink.clone();
        let visible = visible.clone();
        columns = columns.on_scroll_wheel(move |event, window, cx| {
            if !event.modifiers.platform {
                return;
            }
            cx.stop_propagation();
            let dy = f32::from(event.delta.pixel_delta(window.line_height()).y);
            let at = time_at(event.position.x, &visible, &sink.get());
            let factor = 0.99f64.powf(f64::from(dy));
            view.update(cx, |this, cx| this.histogram_zoom(at, factor, cx));
        });
    }
    {
        let span = visible.end - visible.start;
        let edge = move |sink: &Bounds<Pixels>| f64::from(EDGE_PX / f32::from(sink.size.width).max(1.0)) * span;
        let (v_down, v_move, v_up, v_up_out) = (view.clone(), view.clone(), view.clone(), view.clone());
        let (s_down, s_move) = (bounds_sink.clone(), bounds_sink.clone());
        let (vis_down, vis_move) = (visible.clone(), visible.clone());
        let down_order = order.clone();
        columns = columns
            .cursor_crosshair()
            .on_mouse_down(MouseButton::Left, move |event, _, cx| {
                let bounds = s_down.get();
                let at = time_at(event.position.x, &vis_down, &bounds);
                let x = f32::from(event.position.x);
                // Below the columns: the lane pressed on.
                let below = f32::from(event.position.y) - f32::from(bounds.origin.y) - bars;
                let lane = (below >= 0.0).then(|| down_order.get((below / LANE_HEIGHT) as usize).copied()).flatten();
                v_down.update(cx, |this, cx| this.histogram_mouse_down(at, x, edge(&bounds), lane, cx));
            })
            .on_mouse_move(move |event, _, cx| {
                if event.pressed_button != Some(MouseButton::Left) {
                    return;
                }
                let at = time_at(event.position.x, &vis_move, &s_move.get());
                let x = f32::from(event.position.x);
                v_move.update(cx, |this, cx| {
                    if this.histogram.drag.is_some() {
                        this.histogram_mouse_drag(at, x, cx)
                    }
                });
            })
            .on_mouse_up(MouseButton::Left, move |_, _, cx| v_up.update(cx, |this, cx| this.histogram_mouse_up(cx)))
            .on_mouse_up_out(MouseButton::Left, move |_, _, cx| v_up_out.update(cx, |this, cx| this.histogram_mouse_up(cx)));
    }

    div()
        .w_full()
        .flex()
        .flex_col()
        .border_b_1()
        .border_color(colors.border)
        .bg(colors.background)
        .child(labels(&view, &axis, &buckets, collapsed, &colors, &lane_menu, !lanes.is_empty(), cx))
        .child(
            div()
                .px(px(8.0))
                .flex()
                .flex_row()
                .when(!lanes.is_empty(), |row| row.child(lane_names(&view, &lanes, bars, &colors)))
                .child(columns),
        )
        .into_any_element()
}

/// The gutter of lane names, level with the lanes: clicking a value's name
/// ticks it in the Filter.
fn lane_names(view: &Entity<AppView>, lanes: &[(Lane, String)], bars: f32, colors: &CanvasColors) -> impl IntoElement {
    let mut names = div().w(px(NAMES_WIDTH)).flex_shrink_0().flex().flex_col().child(div().h(px(bars)));
    for (i, (lane, name)) in lanes.iter().enumerate() {
        let (lane, name) = (*lane, name.clone());
        let is_value = matches!(lane, Lane::Value(_));
        let v = view.clone();
        let tip = (name.chars().count() > NAME_CHARS).then(|| name.clone());
        names = names.child(
            div()
                .id(("lane_name", i))
                .h(px(LANE_HEIGHT))
                .pr(px(6.0))
                .flex()
                .items_center()
                .text_size(px(10.0))
                .text_color(if is_value { colors.header_fg } else { colors.muted })
                .child(div().min_w_0().truncate().child(name))
                .when(is_value, |d| {
                    d.cursor_pointer()
                        .hover(|s| s.underline())
                        .on_click(move |_, _, cx| v.update(cx, |this, cx| this.lane_name_clicked(lane, cx)))
                })
                .when_some(tip, |d, tip| {
                    d.tooltip(move |window, cx| gpui_component::tooltip::Tooltip::new(tip.clone()).build(window, cx))
                }),
        );
    }
    names
}

/// The "Lane by" button and its menu: "No split", then every Filterable
/// Column (the suggested one first), those that cannot be chosen greyed with
/// the reason.
fn lane_by_button(view: &Entity<AppView>, menu: &LaneMenu) -> impl IntoElement {
    // Over the lane names there is room for the column's name alone.
    let text = match &menu.current {
        Some((_, label)) => format!("{label} ▾"),
        None => "Lane by ▾".to_string(),
    };
    let current = menu.current.as_ref().map(|(k, _)| k.clone());
    let choices = Rc::new(menu.choices.clone());
    let suggested = menu.suggested.clone();
    let view = view.clone();
    Button::new("lane_by").ghost().xsmall().label(text).tooltip("Lane by").dropdown_menu(move |mut popup, _, _| {
        let none = view.clone();
        popup = popup.item(
            PopupMenuItem::new("No split")
                .checked(current.is_none())
                .on_click(move |_, _, cx| none.update(cx, |this, cx| this.set_lane_by(crate::time_histogram::LaneBy::None, cx))),
        );
        popup = popup.item(PopupMenuItem::separator());
        for choice in choices.iter() {
            let mut label = choice.label.clone();
            if suggested.as_deref() == Some(choice.key.as_str()) {
                label.push_str(" (suggested)");
            }
            if let Some(why) = &choice.unavailable {
                label = format!("{label} — {why}");
            }
            let v = view.clone();
            let key = choice.key.clone();
            popup = popup.item(
                PopupMenuItem::new(label)
                    .checked(current.as_deref() == Some(choice.key.as_str()))
                    .disabled(choice.unavailable.is_some())
                    .on_click(move |_, _, cx| {
                        let key = key.clone();
                        v.update(cx, |this, cx| this.set_lane_by(crate::time_histogram::LaneBy::Column(key), cx))
                    }),
            );
        }
        popup
    })
}

/// The row above the columns: the axis's first and last time, the lines
/// with no time, "Show all" while zoomed, and the fold toggle.
#[allow(clippy::too_many_arguments)]
fn labels(
    view: &Entity<AppView>,
    axis: &TimeAxis,
    buckets: &Buckets,
    collapsed: bool,
    colors: &CanvasColors,
    lane_menu: &LaneMenu,
    has_lanes: bool,
    _cx: &App,
) -> impl IntoElement {
    let span = axis.visible.end - axis.visible.start;
    let small = |text: String| div().text_size(px(10.0)).text_color(colors.muted).child(text);
    let link = |id: &'static str, text: String| {
        div().id(id).text_size(px(10.0)).text_color(colors.info).cursor_pointer().hover(|s| s.underline()).child(text)
    };
    let mut row = div()
        .h(px(LABELS_HEIGHT))
        .px(px(8.0))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.0))
        // Over the lane names when there are lanes, so the first time sits
        // above the first column.
        .child(div().when(has_lanes, |d| d.w(px(NAMES_WIDTH - 10.0)).flex_shrink_0()).child(lane_by_button(view, lane_menu)))
        .child(small(label(axis.visible.start, span)))
        .child(div().flex_1());
    if buckets.untimed > 0 {
        let v = view.clone();
        let n = crate::app::format_count(buckets.untimed);
        let text = if buckets.untimed == 1 { "1 line with no time".to_string() } else { format!("{n} lines with no time") };
        row = row.child(link("histogram_untimed", text).on_click(move |_, _, cx| v.update(cx, |this, cx| this.histogram_jump_to_untimed(cx))));
    }
    if !axis.shows_all() {
        let v = view.clone();
        row = row.child(link("histogram_show_all", "Show all".into()).on_click(move |_, _, cx| v.update(cx, |this, cx| this.histogram_show_all(cx))));
    }
    {
        let v = view.clone();
        let text = if collapsed { "▾" } else { "▴" };
        row = row.child(
            div()
                .id("histogram_fold")
                .text_size(px(10.0))
                .text_color(colors.muted)
                .cursor_pointer()
                .child(text)
                .on_click(move |_, _, cx| v.update(cx, |this, cx| this.toggle_histogram_collapsed(cx))),
        );
    }
    row.child(small(label(axis.visible.end, span)))
}

/// The Time Range a drag under way would set, to draw while dragging.
pub fn drag_preview(app: &AppView) -> Option<(f64, f64)> {
    let drag = app.histogram.drag?;
    if matches!(drag, HistogramDrag::Pressed { .. }) {
        return None;
    }
    app.histogram_drag_range(drag)
}
