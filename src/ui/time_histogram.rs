//! The Time Histogram strip (openspec change `add-time-histogram`, decision
//! 4): one column per span of time, painted with `gpui::canvas` like the
//! minimap. From the bottom up: the Time Range's background, the columns
//! (all lines dim under a Filter, the lines passing it solid), the ticks of
//! the columns with a Match, and the frame of the part the list shows.
//! Under it, a strip of its own on the same axis, the Swimlanes (openspec
//! change `add-swimlane`, decision 6): a row of bars per value, with the
//! names on the left; the histogram keeps the same room so both line up.

use crate::app::AppView;
use crate::app::time_histogram::HistogramDrag;
use crate::app::swimlane::LaneChoice;
use crate::time_histogram::{Buckets, Grid, Lane, Lanes, LevelClass, TimeAxis};
use crate::ui::canvas::CanvasColors;
use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, Bounds, Entity, InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels, StatefulInteractiveElement,
    Styled, Window, div, px,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_component::{ActiveTheme as _, Sizable as _};
use gpui_kit::assets::IconName;
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
/// Height of a lane's mark where it has only lines of no note.
const LANE_QUIET: f32 = 2.0;
/// Width of the lane names on the left; the columns start after it.
const NAMES_WIDTH: f32 = 90.0;

/// The "Lane by" menu.
pub struct LaneMenu {
    /// The column the lanes split by now, as its label.
    pub current: Option<(String, String)>,
    pub choices: Vec<LaneChoice>,
    /// The key of the column suggested, when the view has no lanes.
    pub suggested: Option<String>,
}

/// What both strips are drawn on: the view's axis and what lies over it.
pub struct StripProps {
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
    /// Receives where the strip's axis is drawn.
    pub bounds_sink: Rc<Cell<Bounds<Pixels>>>,
}

/// What the Time Histogram strip shows.
pub struct HistogramProps {
    pub strip: StripProps,
    pub collapsed: bool,
    /// The Swimlane strip is shown under it: keep the room of its names on
    /// the left, so both axes line up.
    pub gutter: bool,
}

/// A lane as its name shows it.
pub struct LaneRow {
    pub lane: Lane,
    pub name: String,
    /// What its tooltip says.
    pub title: String,
    /// The color of the file it stands for, by Source or Log Name.
    pub tint: Option<gpui::Hsla>,
    /// Its value is ticked in the Filter.
    pub ticked: bool,
}

/// What the Swimlane strip shows.
pub struct LanesProps {
    pub strip: StripProps,
    pub collapsed: bool,
    /// The lanes drawn, in order.
    pub lanes: Vec<LaneRow>,
    pub menu: LaneMenu,
    /// The Time Histogram is hidden: the strip shows the axis's times.
    pub show_times: bool,
    /// What the pointer is over, said in the header.
    pub hover: Option<String>,
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

/// The color a class is drawn in: Error red, Warn orange, the rest grey at
/// `quiet` opacity.
fn class_fill(colors: &CanvasColors, class: LevelClass, quiet: f32) -> gpui::Hsla {
    match class {
        LevelClass::Error => colors.danger,
        LevelClass::Warn => colors.warning,
        LevelClass::Other => colors.muted.opacity(quiet),
    }
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
    let fills = [LevelClass::Other, LevelClass::Warn, LevelClass::Error].map(|c| class_fill(colors, c, 0.55));
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
    for (i, lane) in order.iter().enumerate() {
        let y0 = top + i as f32 * LANE_HEIGHT;
        window.paint_quad(gpui::fill(rect(f32::from(bounds.origin.x), y0, f32::from(bounds.right()), y0 + 0.5), colors.border.opacity(0.5)));
        let mid = y0 + LANE_HEIGHT / 2.0;
        for (col, class) in lanes.classes(*lane).enumerate() {
            let Some(class) = class else { continue };
            let x0 = x_of(grid.start_of(col), visible, bounds).max(f32::from(bounds.origin.x));
            let x1 = x_of(grid.start_of(col + 1), visible, bounds).min(f32::from(bounds.right()));
            let x1 = if x1 - x0 > 3.0 { x1 - 1.0 } else { x1.max(x0 + 1.0) };
            // Lines of no note are a thin quiet mark, so a Warn or an Error
            // stands out at a glance.
            let half = if class == LevelClass::Other { LANE_QUIET / 2.0 } else { LANE_BAR / 2.0 };
            window.paint_quad(gpui::fill(rect(x0, mid - half, x1, mid + half), class_fill(colors, class, 0.35)));
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

/// The area of a strip drawn on the axis, `height` tall: the Time Range's
/// background, `body`, the part not read yet dimmed and the frame of the
/// part the list shows; ⌘ + scroll zooms, a press, drag or click goes to the
/// histogram's handlers. With `lanes`, a press says which lane it is on.
fn axis_area(
    strip: &StripProps,
    id: &'static str,
    height: f32,
    lanes: Option<Rc<Vec<Lane>>>,
    body: impl Fn(&Bounds<Pixels>, &mut Window) + 'static,
) -> gpui::Stateful<gpui::Div> {
    let StripProps { view, axis, time_range, drag_range, indexed_to, colors, bounds_sink, .. } = strip;
    let (time_range, drag_range, indexed_to) = (*time_range, *drag_range, *indexed_to);
    let visible = axis.visible.clone();
    let paint_view = view.clone();
    let sink = bounds_sink.clone();
    let paint_visible = visible.clone();
    let paint_colors = colors.clone();
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
            if let Some(range) = drag_range.map(|(a, b)| (Some(a), Some(b))).or(time_range) {
                paint_range(window, range, visible, &bounds, colors, drag_range.is_some());
            }
            body(&bounds, window);
            if let Some(read_to) = indexed_to.filter(|t| *t < visible.end) {
                // Not counted yet: dim.
                let x = x_of(read_to, visible, &bounds).max(f32::from(bounds.origin.x));
                window.paint_quad(gpui::fill(
                    rect(x, f32::from(bounds.origin.y), f32::from(bounds.right()), f32::from(bounds.bottom())),
                    colors.background.opacity(0.6),
                ));
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

    let mut area = div().id(id).relative().flex_1().min_w_0().h(px(height)).child(canvas);
    // ⌘ + scroll zooms around the pointer.
    {
        let view = view.clone();
        let sink = bounds_sink.clone();
        let visible = visible.clone();
        area = area.on_scroll_wheel(move |event, window, cx| {
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
    let span = visible.end - visible.start;
    let edge = move |sink: &Bounds<Pixels>| f64::from(EDGE_PX / f32::from(sink.size.width).max(1.0)) * span;
    let (v_down, v_move, v_up, v_up_out, v_leave) = (view.clone(), view.clone(), view.clone(), view.clone(), view.clone());
    let (s_down, s_move) = (bounds_sink.clone(), bounds_sink.clone());
    let (vis_down, vis_move) = (visible.clone(), visible);
    let move_lanes = lanes.clone();
    let lane_at = |order: &[Lane], y: Pixels, bounds: &Bounds<Pixels>| {
        let below = f32::from(y) - f32::from(bounds.origin.y);
        (below >= 0.0).then(|| order.get((below / LANE_HEIGHT) as usize).copied()).flatten()
    };
    area.cursor_crosshair()
        .when(lanes.is_some(), |d| {
            d.on_hover(move |hovered, _, cx| {
                if !*hovered {
                    v_leave.update(cx, |this, cx| this.lane_hover_clear(cx));
                }
            })
        })
        .on_mouse_down(MouseButton::Left, move |event, _, cx| {
            let bounds = s_down.get();
            let at = time_at(event.position.x, &vis_down, &bounds);
            let x = f32::from(event.position.x);
            let lane = lanes.as_ref().and_then(|order| lane_at(order, event.position.y, &bounds));
            v_down.update(cx, |this, cx| this.histogram_mouse_down(at, x, edge(&bounds), lane, cx));
        })
        .on_mouse_move(move |event, _, cx| {
            let bounds = s_move.get();
            let at = time_at(event.position.x, &vis_move, &bounds);
            if event.pressed_button != Some(MouseButton::Left) {
                // Over a lane: say what is there.
                let lane = move_lanes.as_ref().and_then(|order| lane_at(order, event.position.y, &bounds));
                if let Some(lane) = lane {
                    v_move.update(cx, |this, cx| this.lane_hover_at(lane, at, cx));
                } else if move_lanes.is_some() {
                    v_move.update(cx, |this, cx| this.lane_hover_clear(cx));
                }
                return;
            }
            let x = f32::from(event.position.x);
            v_move.update(cx, |this, cx| {
                if this.histogram.drag.is_some() {
                    this.histogram_mouse_drag(at, x, cx)
                }
            });
        })
        .on_mouse_up(MouseButton::Left, move |_, _, cx| v_up.update(cx, |this, cx| this.histogram_mouse_up(cx)))
        .on_mouse_up_out(MouseButton::Left, move |_, _, cx| v_up_out.update(cx, |this, cx| this.histogram_mouse_up(cx)))
}

/// The Time Histogram strip: its labels, then its columns.
pub fn render(props: HistogramProps, _cx: &App) -> gpui::AnyElement {
    let HistogramProps { strip, collapsed, gutter } = props;
    let bars = if collapsed { FOLDED_HEIGHT } else { BARS_HEIGHT };
    let grid = strip.buckets.grid.unwrap_or_else(|| strip.axis.grid());
    let (buckets, visible, colors) = (strip.buckets.clone(), strip.axis.visible.clone(), strip.colors.clone());
    let columns = axis_area(&strip, "time_histogram", bars, None, move |bounds, window| {
        let (top, bottom) = (f32::from(bounds.origin.y) + 2.0, f32::from(bounds.bottom()) - TICK_HEIGHT - 1.0);
        paint_columns(window, &buckets, &grid, &visible, bounds, (top, bottom), &colors);
    });
    let colors = &strip.colors;
    div()
        .w_full()
        .flex()
        .flex_col()
        .border_b_1()
        .border_color(colors.border)
        .bg(colors.background)
        .child(labels(&strip, collapsed, gutter))
        .child(
            div()
                .px(px(8.0))
                .flex()
                .flex_row()
                .when(gutter, |row| row.child(div().w(px(NAMES_WIDTH)).flex_shrink_0()))
                .child(columns),
        )
        .into_any_element()
}

/// The Swimlane strip under the histogram, on the same axis: a header with
/// the "Lane by" menu and its own fold toggle, then a row of bars per lane
/// with the names on the left.
pub fn render_lanes(props: LanesProps, cx: &App) -> gpui::AnyElement {
    let LanesProps { strip, collapsed, lanes, menu, show_times, hover } = props;
    let colors = strip.colors.clone();
    let order: Rc<Vec<Lane>> = Rc::new(lanes.iter().map(|l| l.lane).collect());
    let small = |text: String| div().text_size(px(10.0)).text_color(colors.muted).child(text);
    let span = strip.axis.visible.end - strip.axis.visible.start;
    let mut header = div()
        .h(px(LABELS_HEIGHT))
        .px(px(8.0))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.0))
        .child(div().w(px(NAMES_WIDTH - 10.0)).flex_shrink_0().child(lane_by_button(&strip.view, &menu)));
    header = if show_times { header.child(small(label(strip.axis.visible.start, span))) } else { header };
    header = header.child(div().flex_1());
    if let Some(text) = hover {
        header = header.child(div().text_size(px(10.0)).text_color(cx.theme().foreground).truncate().child(text));
    }
    if show_times {
        header = header.child(small(label(strip.axis.visible.end, span)));
    }
    let v = strip.view.clone();
    header = header.child(fold_button("lanes_fold", collapsed, move |cx| v.update(cx, |this, cx| this.toggle_lanes_collapsed(cx))));
    let mut out = div().w_full().flex().flex_col().border_b_1().border_color(colors.border).bg(colors.background).child(header);
    if collapsed {
        return out.into_any_element();
    }
    match (strip.buckets.lanes.as_ref(), lanes.is_empty()) {
        (Some(_), false) => {
            let grid = strip.buckets.grid.unwrap_or_else(|| strip.axis.grid());
            let (buckets, visible, paint_colors, paint_order) =
                (strip.buckets.clone(), strip.axis.visible.clone(), colors.clone(), order.clone());
            let height = order.len() as f32 * LANE_HEIGHT + 2.0;
            let area = axis_area(&strip, "swimlanes", height, Some(order), move |bounds, window| {
                if let Some(lanes) = buckets.lanes.as_ref() {
                    let top = f32::from(bounds.origin.y) + 1.0;
                    paint_lanes(window, lanes, &paint_order, &grid, &visible, bounds, top, &paint_colors);
                }
            });
            out = out.child(
                div()
                    .px(px(8.0))
                    .pb(px(2.0))
                    .flex()
                    .flex_row()
                    .child(lane_names(&strip.view, &lanes, &colors, cx))
                    .child(area),
            );
        }
        _ => {
            // No lanes: say why, and what the menu suggests.
            let text = match &menu.suggested {
                Some(key) => {
                    let name = menu.choices.iter().find(|c| &c.key == key).map_or(key.as_str(), |c| c.label.as_str());
                    format!("No split. Lane by {name} to see each one on its own row.")
                }
                None => "No split. Choose a column in Lane by.".to_string(),
            };
            out = out.child(div().px(px(8.0)).pb(px(4.0)).child(small(text)));
        }
    }
    out.into_any_element()
}

/// The gutter of lane names, level with the lanes: a value's name in the
/// color of its file when it stands for one, marked when ticked in the
/// Filter; clicking it ticks or unticks it there.
fn lane_names(view: &Entity<AppView>, lanes: &[LaneRow], colors: &CanvasColors, cx: &App) -> impl IntoElement {
    let mut names = div().w(px(NAMES_WIDTH)).flex_shrink_0().flex().flex_col().pt(px(1.0));
    for (i, row) in lanes.iter().enumerate() {
        let lane = row.lane;
        let is_value = matches!(lane, Lane::Value(_));
        let v = view.clone();
        let title = row.title.clone();
        // Ticked is the active state (design-system): accent behind, the
        // foreground text.
        let text = match (is_value, row.ticked) {
            (true, true) => cx.theme().foreground,
            (true, false) => row.tint.unwrap_or(cx.theme().foreground),
            (false, _) => colors.muted,
        };
        let (accent, hover) = (cx.theme().accent, cx.theme().list_hover);
        names = names.child(
            div()
                .id(("lane_name", i))
                .h(px(LANE_HEIGHT))
                .pl(px(4.0))
                .pr(px(6.0))
                .flex()
                .items_center()
                .rounded(px(3.0))
                .text_size(px(11.0))
                .text_color(text)
                .when(row.ticked, |d| d.bg(accent))
                .child(div().min_w_0().truncate().child(row.name.clone()))
                .when(is_value, |d| {
                    d.cursor_pointer()
                        .when(!row.ticked, |d| d.hover(move |s| s.bg(hover)))
                        .on_click(move |_, _, cx| v.update(cx, |this, cx| this.lane_name_clicked(lane, cx)))
                })
                .tooltip(move |window, cx| {
                    let tip = if is_value { format!("{title}: click to filter by it") } else { title.clone() };
                    gpui_component::tooltip::Tooltip::new(tip).build(window, cx)
                }),
        );
    }
    names
}

/// The fold toggle of a strip: a chevron with room to press.
fn fold_button(id: &'static str, collapsed: bool, toggle: impl Fn(&mut App) + 'static) -> impl IntoElement {
    Button::new(id)
        .ghost()
        .xsmall()
        .icon(if collapsed { IconName::ChevronRight } else { IconName::ChevronDown })
        .tooltip(if collapsed { "Open" } else { "Fold" })
        .on_click(move |_, _, cx| toggle(cx))
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
fn labels(strip: &StripProps, collapsed: bool, gutter: bool) -> impl IntoElement {
    let StripProps { view, axis, buckets, colors, .. } = strip;
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
        // Over the lane names' room, so the first time sits above the first
        // column.
        .when(gutter, |d| d.child(div().w(px(NAMES_WIDTH - 10.0)).flex_shrink_0()))
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
    let row = row.child(small(label(axis.visible.end, span)));
    let v = view.clone();
    row.child(fold_button("histogram_fold", collapsed, move |cx| v.update(cx, |this, cx| this.toggle_histogram_collapsed(cx))))
}

/// The Time Range a drag under way would set, to draw while dragging.
pub fn drag_preview(app: &AppView) -> Option<(f64, f64)> {
    let drag = app.histogram.drag?;
    if matches!(drag, HistogramDrag::Pressed { .. }) {
        return None;
    }
    app.histogram_drag_range(drag)
}
