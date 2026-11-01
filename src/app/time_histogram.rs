//! Logic: the Time Histogram of the open view (openspec change
//! `add-time-histogram`): its axis, its counts done on a worker thread, and
//! what clicking and dragging on it does.
//!
//! Every path that changes what the histogram counts (rows, Filter, the
//! Matches, the Focus Tab's lines, the axis) ends in `refresh_histogram`,
//! which compares what the last count was for with what there is now: rows
//! only added at the end are added to the columns, anything else is counted
//! again on a worker thread.

use super::AppView;
use super::focus::Origin;
use crate::index::file::IndexedFile;
use crate::index::view_rows::ViewRows;
use crate::time_histogram::{self as th, Buckets, RowFacts, TimeAxis};
use crate::workspace::working_set::{TabTarget, ViewMode};
use gpui::Context;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Lines arriving redraw the histogram at most this often.
const APPEND_EVERY: Duration = Duration::from_millis(500);

/// Width assumed before the histogram was first drawn.
const DEFAULT_WIDTH: f32 = 800.0;

/// What a count was for: when it differs from what there is now, count again.
#[derive(Debug, Clone, PartialEq)]
struct CountKey {
    rows_epoch: u64,
    rows: usize,
    /// The Filter result (its list and length), with a Filter in effect.
    pass: Option<(usize, usize)>,
    matches: (usize, usize),
    grid: th::Grid,
    full: Range<f64>,
    /// The column the lanes split by.
    lanes: Option<String>,
}

impl CountKey {
    /// Whether `self` only has more rows than `before`, added at the end,
    /// with the same lists grown in place.
    fn only_appended_to(&self, before: &CountKey) -> bool {
        self.rows_epoch == before.rows_epoch
            && self.rows > before.rows
            && self.grid == before.grid
            && self.full == before.full
            && self.lanes == before.lanes
            && self.matches.0 == before.matches.0
            && self.matches.1 >= before.matches.1
            && match (self.pass, before.pass) {
                (Some(now), Some(then)) => now.0 == then.0 && now.1 >= then.1,
                (None, None) => true,
                _ => false,
            }
    }
}

/// A drag on the histogram, in times on the axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HistogramDrag {
    /// Pressed, not moved yet: a click unless it moves. `lane` is the
    /// Swimlane pressed on, below the columns.
    Pressed { at: f64, x: f32, inside_range: bool, lane: Option<th::Lane> },
    /// Choosing a new Time Range.
    Select { from: f64, to: f64 },
    /// Moving one edge of the Time Range; the other stays.
    Edge { fixed: f64, moving: f64 },
    /// Moving the whole Time Range by the distance from `grab`.
    Move { range: (f64, f64), grab: f64, now: f64 },
}

/// The Time Histogram of the open view.
pub struct HistogramState {
    /// The axis of the view, shared by its histogram and the Focus Pane's.
    pub axis: Option<TimeAxis>,
    /// The tab the axis is for: another tab starts from its own whole span.
    axis_for: Option<TabTarget>,
    /// The counts of the view's rows.
    pub shown: Arc<Buckets>,
    /// The column the shown lanes are for.
    pub lanes_for: Option<String>,
    /// Each row's class (`time_histogram::class_of`), built with the counts.
    classes: Arc<Vec<u8>>,
    /// `rows_epoch` the classes are for.
    classes_epoch: Option<u64>,
    /// What the shown counts are for, and what the count under way is for.
    counted: Option<CountKey>,
    asked: Option<CountKey>,
    /// The newest count asked for, and the newest shown.
    generation: u64,
    applied: u64,
    /// When appended lines were last added, and whether a later redraw is
    /// waiting.
    appended_at: Option<Instant>,
    append_waiting: bool,
    /// Where the view's histogram was drawn, written while painting.
    pub bounds: Rc<std::cell::Cell<gpui::Bounds<gpui::Pixels>>>,
    /// While files are read: the time the read lines reach. The axis after
    /// it is drawn dim.
    pub indexed_to: Option<f64>,
    pub drag: Option<HistogramDrag>,
    /// The strip is folded to its thin form.
    pub collapsed: bool,
    /// Where the Swimlane strip's axis was drawn, written while painting.
    pub lanes_bounds: Rc<std::cell::Cell<gpui::Bounds<gpui::Pixels>>>,
    /// The Swimlane strip is folded to its header.
    pub lanes_collapsed: bool,
    /// What the pointer is over on the Swimlanes.
    pub lane_hover: Option<super::swimlane::LaneHover>,
}

impl Default for HistogramState {
    fn default() -> Self {
        Self {
            axis: None,
            axis_for: None,
            shown: Arc::default(),
            lanes_for: None,
            classes: Arc::default(),
            classes_epoch: None,
            counted: None,
            asked: None,
            generation: 0,
            applied: 0,
            appended_at: None,
            append_waiting: false,
            bounds: Rc::default(),
            indexed_to: None,
            drag: None,
            collapsed: false,
            lanes_bounds: Rc::default(),
            lanes_collapsed: false,
            lane_hover: None,
        }
    }
}

/// The class of every row of `rows`, from the files' line metadata: each
/// file is locked once for the whole pass.
fn build_classes(rows: &ViewRows, files: &[Option<Arc<IndexedFile>>]) -> Vec<u8> {
    let guards: Vec<_> = files.iter().map(|f| f.as_ref().map(|f| f.indexed.data.read_recursive())).collect();
    rows.iter()
        .map(|packed| {
            let (file, pos) = crate::index::view_rows::unpack(packed);
            match guards.get(file).and_then(|g| g.as_ref()).and_then(|d| d.file.meta.get(pos)) {
                Some(m) => th::class_of(Some(m.level()), m.parent_dist > 0),
                None => th::class_of(None, false),
            }
        })
        .collect()
}

fn list_key<T>(list: &Arc<Vec<T>>) -> (usize, usize) {
    (Arc::as_ptr(list) as usize, list.len())
}

impl AppView {
    /// Whether the open view has a Time Histogram (decision 8): it is on,
    /// the view is a Parsed View of log lines, and a line has a time.
    pub fn histogram_shown(&self) -> bool {
        self.workspace_state.show_histogram && self.has_time_axis()
    }

    /// Whether the open view has the Swimlane strip: it is on, apart from
    /// the histogram, on a view that has a time axis.
    pub fn lanes_shown(&self) -> bool {
        self.workspace_state.show_lanes && self.has_time_axis()
    }

    /// Whether the open view can show strips on a time axis: a Parsed View
    /// of log lines, one of which has a time.
    fn has_time_axis(&self) -> bool {
        self.view_mode != ViewMode::Raw
            && self.active_tab_is_log_file()
            && self.empty_canvas().is_none()
            && self.filter_index.read(&self.view.rows).timed_lines() > 0
    }

    /// The width of the axis, from whichever strip was drawn.
    fn histogram_width(&self) -> f32 {
        let drawn = |b: &std::cell::Cell<gpui::Bounds<gpui::Pixels>>| f32::from(b.get().size.width);
        let w = if self.histogram_shown() { drawn(&self.histogram.bounds) } else { drawn(&self.histogram.lanes_bounds) };
        if w > 0.0 { w } else { DEFAULT_WIDTH }
    }

    /// The span of the view's times, and while files are read, how far
    /// they are.
    fn histogram_span(&self) -> Option<(Range<f64>, Option<f64>)> {
        let (first, last) = self.filter_index.read(&self.view.rows).time_span()?;
        // A file still read: the axis reaches as far as the part read so far
        // suggests the whole will.
        let reading: Vec<u8> = self
            .view
            .files
            .iter()
            .filter_map(|f| self.loads.get(&*f.file_id))
            .map(|l| l.percent())
            .collect();
        if reading.is_empty() {
            return Some((th::pad_span(first..last), None));
        }
        let percent = (reading.iter().map(|&p| u32::from(p)).sum::<u32>() / reading.len() as u32).clamp(1, 100);
        let end = first + (last - first) * 100.0 / f64::from(percent);
        Some((th::pad_span(first..end.max(last)), Some(last)))
    }

    /// Bring the histogram up to date with the view: grow the axis to the
    /// rows' times, add rows appended at the end, or count again on a
    /// worker thread. Cheap when nothing changed.
    pub fn refresh_histogram(&mut self, cx: &mut Context<Self>) {
        if !self.histogram_shown() && !self.lanes_shown() {
            return;
        }
        let Some((span, indexed_to)) = self.histogram_span() else { return };
        self.histogram.indexed_to = indexed_to;
        let width = self.histogram_width();
        let target = self.active_target();
        let axis = match self.histogram.axis.take() {
            Some(mut axis) if self.histogram.axis_for == target => {
                if span.start < axis.full.start || span.end > axis.full.end {
                    let shows_all = axis.shows_all();
                    let grown = th::grow_full(&th::grow_full(&axis.full, span.start), span.end);
                    // While reading, the axis is the estimate itself.
                    axis.full = if indexed_to.is_some() { span.start.min(grown.start)..span.end } else { grown };
                    let visible = if shows_all { axis.full.clone() } else { axis.visible.clone() };
                    axis.set_visible(visible, width);
                } else if th::Step::for_width(&axis.visible, width) != axis.step {
                    let visible = axis.visible.clone();
                    axis.set_visible(visible, width);
                }
                axis
            }
            _ => {
                self.histogram.axis_for = target;
                self.histogram.counted = None;
                self.histogram.asked = None;
                self.histogram.drag = None;
                TimeAxis::new(span, width)
            }
        };
        let key = CountKey {
            rows_epoch: self.rows_epoch,
            rows: self.view.len(),
            pass: (!self.filter_state.is_empty()).then(|| self.filter_pass.key()),
            matches: list_key(&self.focus.search().matched_rows),
            grid: axis.grid(),
            full: axis.full.clone(),
            // Lanes are counted only while their strip shows.
            lanes: self.lanes_shown().then(|| self.lane_column()).flatten(),
        };
        self.histogram.axis = Some(axis);
        if self.histogram.asked.as_ref() == Some(&key) {
            return;
        }
        if let Some(counted) = self.histogram.counted.clone().filter(|c| key.only_appended_to(c)) {
            if self.histogram.asked.as_ref() == Some(&counted) {
                self.histogram_append(counted, key, cx);
                return;
            }
        }
        self.schedule_histogram_recount(key, cx);
    }

    /// Rows were added after those counted (`counted`): add them to the
    /// columns, at most twice a second; count again when one falls outside
    /// the axis.
    fn histogram_append(&mut self, counted: CountKey, key: CountKey, cx: &mut Context<Self>) {
        let now = Instant::now();
        if let Some(at) = self.histogram.appended_at.filter(|at| now.duration_since(*at) < APPEND_EVERY) {
            if !self.histogram.append_waiting {
                self.histogram.append_waiting = true;
                let wait = APPEND_EVERY - now.duration_since(at);
                cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(wait).await;
                    this.update(cx, |this, cx| {
                        this.histogram.append_waiting = false;
                        this.refresh_histogram(cx);
                        cx.notify();
                    })
                    .ok();
                })
                .detach();
            }
            return;
        }
        let from = counted.rows;
        if self.histogram.classes.len() != from || self.histogram.classes_epoch != Some(self.rows_epoch) {
            return self.schedule_histogram_recount(key, cx);
        }
        let added: Vec<u8> = {
            let rows = self.rows();
            (from..key.rows).map(|r| th::class_of(rows.level(r), rows.continues(r))).collect()
        };
        Arc::make_mut(&mut self.histogram.classes).extend(added);
        let pass = key.pass.map(|_| self.filter_pass.from_row(from));
        let matches = &self.focus.search().matched_rows;
        let matches = &matches[matches.partition_point(|&r| r < from)..];
        let index = self.filter_index.read(&self.view.rows);
        let facts = RowFacts { ts: &index, class: &self.histogram.classes };
        let lanes = key.lanes.as_deref().and_then(|k| super::swimlane::lane_ids(&index, k));
        if key.lanes.is_some() && lanes.is_none() {
            drop(index);
            return self.schedule_histogram_recount(key, cx);
        }
        let Some(axis) = self.histogram.axis.as_ref() else { return };
        let lanes = lanes.as_ref().map(|(ids, values)| th::LaneIds { ids, values: *values });
        let appended = Arc::make_mut(&mut self.histogram.shown).append(facts, axis, from, pass, matches, lanes);
        drop(index);
        if appended.is_err() {
            return self.schedule_histogram_recount(key, cx);
        }
        self.histogram.appended_at = Some(now);
        self.histogram.counted = Some(key.clone());
        self.histogram.asked = Some(key);
        cx.notify();
    }

    /// Count everything again on a worker thread, for `key`. A count asked
    /// for after it makes its result stale.
    fn schedule_histogram_recount(&mut self, key: CountKey, cx: &mut Context<Self>) {
        let Some(axis) = self.histogram.axis.clone() else { return };
        self.histogram.generation += 1;
        let generation = self.histogram.generation;
        self.histogram.asked = Some(key.clone());
        let (index, view_rows) = (self.filter_index.clone(), self.view.rows.clone());
        // The classes are built again when the rows changed other than at the end.
        let classes = (self.histogram.classes_epoch == Some(self.rows_epoch) && self.histogram.classes.len() == key.rows)
            .then(|| self.histogram.classes.clone());
        let rebuild = classes.is_none().then(|| {
            let files: Vec<Option<Arc<IndexedFile>>> = self
                .view
                .files
                .iter()
                .map(|f| self.parsed_files.get(&*f.file_id).and_then(|p| p.indexed.clone()))
                .collect();
            (self.view.rows.clone(), files)
        });
        let pass = key.pass.map(|_| self.filter_pass.clone());
        let matches = self.focus.search().matched_rows.clone();
        let lane_key = key.lanes.clone();
        let epoch = self.rows_epoch;
        cx.spawn(async move |this, cx| {
            let counted = cx
                .background_executor()
                .spawn(async move {
                    let classes = classes.unwrap_or_else(|| {
                        let (rows, files) = rebuild.expect("rows to build the classes from");
                        Arc::new(build_classes(&rows, &files))
                    });
                    let index = index.read(&view_rows);
                    let facts = RowFacts { ts: &index, class: &classes };
                    // Each row's value is read through the view's index, like its time.
                    let lanes = lane_key.as_deref().and_then(|k| super::swimlane::lane_ids(&index, k));
                    let lanes = lanes.as_ref().map(|(ids, values)| th::LaneIds { ids, values: *values });
                    let shown = th::count_buckets(facts, &axis, pass.as_ref().map(|p| p.from_row(0)), &matches, lanes);
                    (classes, shown)
                })
                .await;
            this.update(cx, |this, cx| this.histogram_counted(generation, epoch, key, counted, cx)).ok();
        })
        .detach();
    }

    /// A count is done. It is shown unless a newer one was shown already or
    /// it is not on the axis's columns any more.
    fn histogram_counted(
        &mut self,
        generation: u64,
        epoch: u64,
        key: CountKey,
        (classes, shown): (Arc<Vec<u8>>, Buckets),
        cx: &mut Context<Self>,
    ) {
        let h = &mut self.histogram;
        if generation <= h.applied || h.axis.as_ref().map(|a| a.grid()) != Some(key.grid) || epoch != self.rows_epoch {
            return;
        }
        h.applied = generation;
        h.classes = classes;
        h.classes_epoch = Some(epoch);
        // The lanes are for that column only when they were counted.
        h.lanes_for = shown.lanes.as_ref().and(key.lanes.clone());
        h.shown = Arc::new(shown);
        // Counted again: what the pointer was over is read afresh.
        h.lane_hover = None;
        h.counted = Some(key);
        if generation == h.generation {
            // Rows that came while counting are added now.
            h.asked = h.counted.clone();
            self.refresh_histogram(cx);
        }
        cx.notify();
    }

    /// What both strips draw on: the axis, the counts and what lies over
    /// them; `sink` receives where the strip's axis is drawn.
    fn strip_props(
        &self,
        colors: crate::ui::canvas::CanvasColors,
        sink: Rc<std::cell::Cell<gpui::Bounds<gpui::Pixels>>>,
        cx: &mut Context<Self>,
    ) -> Option<crate::ui::time_histogram::StripProps> {
        Some(crate::ui::time_histogram::StripProps {
            view: cx.entity(),
            axis: self.histogram.axis.clone()?,
            buckets: self.histogram.shown.clone(),
            time_range: self.histogram_time_range(),
            drag_range: crate::ui::time_histogram::drag_preview(self),
            indexed_to: self.histogram.indexed_to,
            colors,
            bounds_sink: sink,
        })
    }

    /// The histogram strip of the view, when it has one.
    pub fn render_time_histogram(
        &self,
        colors: crate::ui::canvas::CanvasColors,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        if !self.histogram_shown() {
            return None;
        }
        let props = crate::ui::time_histogram::HistogramProps {
            strip: self.strip_props(colors, self.histogram.bounds.clone(), cx)?,
            collapsed: self.histogram.collapsed,
            gutter: self.lanes_shown(),
        };
        Some(crate::ui::time_histogram::render(props, cx))
    }

    /// The Swimlane strip of the view, when it has one.
    pub fn render_swimlanes(
        &self,
        colors: crate::ui::canvas::CanvasColors,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        if !self.lanes_shown() {
            return None;
        }
        let lanes = self
            .histogram
            .shown
            .lanes
            .as_ref()
            .map(|l| {
                l.order()
                    .into_iter()
                    .map(|lane| crate::ui::time_histogram::LaneRow {
                        lane,
                        name: self.lane_name(lane),
                        title: self.lane_title(lane),
                        tint: self.lane_tint(lane, colors.dark),
                        ticked: self.lane_ticked(lane),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let menu = crate::ui::time_histogram::LaneMenu {
            current: self
                .histogram
                .lanes_for
                .as_deref()
                .and_then(|k| self.filter_index.read(&self.view.rows).column(k).map(|c| (c.key.clone(), c.label.clone()))),
            choices: self.lane_choices(),
            suggested: self.lane_suggestion(),
        };
        let props = crate::ui::time_histogram::LanesProps {
            strip: self.strip_props(colors, self.histogram.lanes_bounds.clone(), cx)?,
            collapsed: self.histogram.lanes_collapsed,
            lanes,
            menu,
            show_times: !self.histogram_shown(),
            hover: self.lane_hover_text(),
        };
        Some(crate::ui::time_histogram::render_lanes(props, cx))
    }

    /// The rows in sight in the list, as the span of their times: the frame
    /// of the part shown. Their times may go back (a File View): the
    /// earliest and latest of them.
    pub fn histogram_in_sight(&self) -> Option<Range<f64>> {
        let row_height = self.text_scale().row_height();
        let height = f32::from(self.canvas_bounds.get().size.height);
        let in_sight = if height > 0.0 { (height / row_height).ceil() as usize } else { 0 };
        let top = self.scroll_handle.0.borrow().base_handle.logical_scroll_top().0;
        let index = self.filter_index.read(&self.view.rows);
        let end = (top + in_sight.max(1)).min(index.len());
        if top > end {
            return None;
        }
        th::span_of((top..end).map(|row| index.ts(row)))
    }

    /// Show the part of the axis from `visible` (zoom); nothing else changes.
    pub fn histogram_set_visible(&mut self, visible: Range<f64>, cx: &mut Context<Self>) {
        let width = self.histogram_width();
        if let Some(axis) = self.histogram.axis.as_mut() {
            axis.set_visible(visible, width);
        }
        self.refresh_histogram(cx);
        cx.notify();
    }

    /// Zoom by `factor` around the time `at` (⌘ + scroll).
    pub fn histogram_zoom(&mut self, at: f64, factor: f64, cx: &mut Context<Self>) {
        let width = self.histogram_width();
        if let Some(axis) = self.histogram.axis.as_mut() {
            axis.zoom(at, factor, width);
        }
        self.refresh_histogram(cx);
        cx.notify();
    }

    /// "Show all": the whole axis.
    pub fn histogram_show_all(&mut self, cx: &mut Context<Self>) {
        if let Some(full) = self.histogram.axis.as_ref().map(|a| a.full.clone()) {
            self.histogram_set_visible(full, cx);
        }
    }

    /// Fold the Swimlane strip to its header, or open it again.
    pub fn toggle_lanes_collapsed(&mut self, cx: &mut Context<Self>) {
        self.histogram.lanes_collapsed = !self.histogram.lanes_collapsed;
        cx.notify();
    }

    /// Fold the strip, or open it again.
    pub fn toggle_histogram_collapsed(&mut self, cx: &mut Context<Self>) {
        self.histogram.collapsed = !self.histogram.collapsed;
        cx.notify();
    }

    /// The Time Range as times, when one is set.
    pub fn histogram_time_range(&self) -> Option<(Option<f64>, Option<f64>)> {
        let range = &self.filter_state.time_range;
        range.is_active().then(|| range.bounds(self.filter_index.read(&self.view.rows).anchor_day()))
    }

    /// Jump to the first row at or after `time`: a column was clicked. The
    /// Filter and the Focus Pane stay as they are.
    pub fn histogram_jump_to(&mut self, time: f64, cx: &mut Context<Self>) {
        let Some(row) = th::first_row_at(&self.filter_index.read(&self.view.rows), time, self.view_times_ascend()) else { return };
        self.histogram_jump_to_row(row, cx);
    }

    /// Jump to the first line with no time (the label in the corner).
    pub fn histogram_jump_to_untimed(&mut self, cx: &mut Context<Self>) {
        if let Some(row) = self.histogram.shown.first_untimed {
            self.histogram_jump_to_row(row, cx);
        }
    }

    pub(crate) fn histogram_jump_to_row(&mut self, row: usize, cx: &mut Context<Self>) {
        if let Some(line) = self.line_ref_of_entry(row) {
            self.set_current_line(line, Origin::Jump, cx);
        }
    }

    /// Set the Time Range to `from`..`to` from the histogram: written in the
    /// From and To fields as the user would type it, then applied as typing
    /// it would be.
    pub fn histogram_set_time_range(&mut self, from: f64, to: f64, cx: &mut Context<Self>) {
        let (from, to) = (from.min(to), from.max(to));
        let step = self.histogram.axis.as_ref().map_or(th::Step(1.0), |a| a.step);
        let (from, to) = th::format_time_range(from, to, self.filter_index.read(&self.view.rows).anchor_day(), step);
        self.set_time_range_text(&from, &to, cx);
    }

    /// Clear the Time Range (a click outside it on the histogram).
    pub fn histogram_clear_time_range(&mut self, cx: &mut Context<Self>) {
        self.set_time_range_text("", "", cx);
    }

    fn set_time_range_text(&mut self, from: &str, to: &str, cx: &mut Context<Self>) {
        // A typed edit waiting for its pause is replaced.
        self.time_range_generation += 1;
        self.filter_state.set_time_range(from, to);
        self.filter_ui.inputs_dirty = true;
        self.apply_filter_change(cx);
    }

    /// The mouse went down on the histogram at time `at`, `x` pixels from
    /// its left, on the Swimlane `lane` if any. Near an edge of the Time
    /// Range it takes that edge, inside it the whole range; else it is a
    /// click until it moves.
    pub fn histogram_mouse_down(&mut self, at: f64, x: f32, edge_px: f64, lane: Option<th::Lane>, cx: &mut Context<Self>) {
        let range = self.histogram_time_range();
        self.histogram.drag = Some(match range {
            Some((Some(from), Some(to))) if (at - from).abs() <= edge_px => HistogramDrag::Edge { fixed: to, moving: at },
            Some((Some(from), Some(to))) if (at - to).abs() <= edge_px => HistogramDrag::Edge { fixed: from, moving: at },
            Some((from, to)) => {
                let inside = from.is_none_or(|f| at >= f) && to.is_none_or(|t| at <= t);
                HistogramDrag::Pressed { at, x, inside_range: inside, lane }
            }
            None => HistogramDrag::Pressed { at, x, inside_range: false, lane },
        });
        cx.notify();
    }

    /// The mouse moved to time `at` (`x` pixels from the left) while down.
    pub fn histogram_mouse_drag(&mut self, at: f64, x: f32, cx: &mut Context<Self>) {
        let Some(drag) = self.histogram.drag else { return };
        self.histogram.drag = Some(match drag {
            HistogramDrag::Pressed { at: start, x: x0, inside_range, .. } if (x - x0).abs() > 3.0 => {
                match (inside_range, self.histogram_time_range()) {
                    (true, Some((Some(from), Some(to)))) => HistogramDrag::Move { range: (from, to), grab: start, now: at },
                    _ => HistogramDrag::Select { from: start, to: at },
                }
            }
            HistogramDrag::Pressed { .. } => drag,
            HistogramDrag::Select { from, .. } => HistogramDrag::Select { from, to: at },
            HistogramDrag::Edge { fixed, .. } => HistogramDrag::Edge { fixed, moving: at },
            HistogramDrag::Move { range, grab, .. } => HistogramDrag::Move { range, grab, now: at },
        });
        cx.notify();
    }

    /// The mouse went up: a drag sets the Time Range once; a click jumps to
    /// its column (on a Swimlane, to the lane's first line there), or
    /// clears the Time Range when outside it.
    pub fn histogram_mouse_up(&mut self, cx: &mut Context<Self>) {
        let Some(drag) = self.histogram.drag.take() else { return };
        match self.histogram_drag_range(drag) {
            Some((from, to)) => self.histogram_set_time_range(from, to, cx),
            None => {
                let HistogramDrag::Pressed { at, inside_range, lane, .. } = drag else { return };
                let lane_col = self.histogram.shown.grid.and_then(|g| g.column_of(at));
                if let (Some(lane), Some(col)) = (lane, lane_col) {
                    self.lane_bar_clicked(lane, col, cx);
                } else if self.histogram_time_range().is_some() && !inside_range {
                    self.histogram_clear_time_range(cx);
                } else if let Some(grid) = self.histogram.axis.as_ref().map(|a| a.grid()) {
                    let start = grid.column_of(at).map_or(at, |c| grid.start_of(c));
                    self.histogram_jump_to(start, cx);
                }
            }
        }
        cx.notify();
    }

    /// The Time Range a drag makes, on whole columns: none for a click.
    pub fn histogram_drag_range(&self, drag: HistogramDrag) -> Option<(f64, f64)> {
        let axis = self.histogram.axis.as_ref()?;
        let step = axis.step.seconds();
        let floor = |t: f64| (t / step).floor() * step;
        let ceil = |t: f64| (t / step).ceil() * step;
        let (from, to) = match drag {
            HistogramDrag::Pressed { .. } => return None,
            HistogramDrag::Select { from, to } => (floor(from.min(to)), ceil(from.max(to))),
            HistogramDrag::Edge { fixed, moving } => {
                let moving = (moving / step).round() * step;
                (fixed.min(moving), fixed.max(moving))
            }
            HistogramDrag::Move { range, grab, now } => {
                let by = ((now - grab) / step).round() * step;
                (range.0 + by, range.1 + by)
            }
        };
        (to > from).then_some((from, to))
    }
}
