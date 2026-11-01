//! Time Histogram: how many Parent Lines fall in each span of time, by
//! level, with no GPUI (openspec change `add-time-histogram`).
//!
//! The axis belongs to the view (decision 2): the view's histogram and the
//! Focus Pane's read the same `TimeAxis`. Counting is one pass over the
//! per-row arrays the view already has (decision 1): the effective time of
//! each row and its level class.

use crate::model::LogLevel;
use std::ops::Range;

/// Widths a column can have, in seconds: 1s, 5s, 10s, 30s, 1m, 5m, 15m, 1h,
/// 6h, 1d.
pub const STEPS: [f64; 10] = [1.0, 5.0, 10.0, 30.0, 60.0, 300.0, 900.0, 3600.0, 21_600.0, 86_400.0];

/// A column is drawn at least this wide, in pixels.
const MIN_COLUMN_PX: f64 = 4.0;

/// The width of one column, in seconds: one of [`STEPS`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Step(pub f64);

impl Step {
    /// The narrowest step whose columns are at least 4 pixels wide across
    /// `width_px` showing `visible`: about 4 to 6 pixels where the table
    /// allows. The widest step when even that is too narrow.
    pub fn for_width(visible: &Range<f64>, width_px: f32) -> Step {
        let span = (visible.end - visible.start).max(0.0);
        let width = f64::from(width_px).max(1.0);
        let least = span * MIN_COLUMN_PX / width;
        Step(STEPS.iter().copied().find(|&s| s >= least).unwrap_or(STEPS[STEPS.len() - 1]))
    }

    pub fn seconds(self) -> f64 {
        self.0
    }
}

/// The time axis of a view.
#[derive(Debug, Clone, PartialEq)]
pub struct TimeAxis {
    /// From the earliest to the latest time of the view (grown with some
    /// room while lines arrive).
    pub full: Range<f64>,
    /// The part shown: `full` unless zoomed.
    pub visible: Range<f64>,
    pub step: Step,
}

impl TimeAxis {
    /// The whole of `full` shown across `width_px`.
    pub fn new(full: Range<f64>, width_px: f32) -> Self {
        let step = Step::for_width(&full, width_px);
        Self { visible: full.clone(), full, step }
    }

    /// The columns of the shown part.
    pub fn grid(&self) -> Grid {
        Grid::new(&self.visible, self.step)
    }

    /// Show `visible`, kept within `full`, and choose the step again for
    /// `width_px`.
    pub fn set_visible(&mut self, visible: Range<f64>, width_px: f32) {
        self.visible = clamp_range(visible, &self.full);
        self.step = Step::for_width(&self.visible, width_px);
    }

    /// Zoom by `factor` (below 1 zooms in) around the time `at`, kept within
    /// `full`.
    pub fn zoom(&mut self, at: f64, factor: f64, width_px: f32) {
        let span = (self.visible.end - self.visible.start) * factor;
        // A column of 1s at least a few pixels wide: no deeper than that.
        let span = span.max(STEPS[0] * f64::from(width_px.max(1.0)) / 10.0).min(self.full.end - self.full.start);
        let left = if self.visible.end > self.visible.start {
            (at - self.visible.start) / (self.visible.end - self.visible.start)
        } else {
            0.5
        };
        let start = at - span * left.clamp(0.0, 1.0);
        self.set_visible(start..start + span, width_px);
    }

    /// Whether the whole of `full` is shown.
    pub fn shows_all(&self) -> bool {
        self.visible == self.full
    }
}

/// Move `r` inside `bounds`, keeping its length where it fits.
fn clamp_range(r: Range<f64>, bounds: &Range<f64>) -> Range<f64> {
    let len = (r.end - r.start).min(bounds.end - bounds.start).max(0.0);
    let start = r.start.clamp(bounds.start, (bounds.end - len).max(bounds.start));
    start..start + len
}

/// The columns over a span of time: column `i` covers
/// `origin + i·step .. origin + (i+1)·step`. The origin is a multiple of the
/// step, so columns start on round times.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grid {
    pub origin: f64,
    pub step: f64,
    pub len: usize,
}

impl Grid {
    pub fn new(span: &Range<f64>, step: Step) -> Self {
        let step = step.seconds();
        let origin = (span.start / step).floor() * step;
        let len = (((span.end - origin) / step).floor() as usize + 1).max(1);
        Self { origin, step, len }
    }

    /// The column holding time `t`, if it is on the grid.
    pub fn column_of(&self, t: f64) -> Option<usize> {
        if !(t >= self.origin) {
            return None;
        }
        let i = ((t - self.origin) / self.step) as usize;
        (i < self.len).then_some(i)
    }

    /// When column `i` starts.
    pub fn start_of(&self, i: usize) -> f64 {
        self.origin + i as f64 * self.step
    }

    /// When the last column ends.
    pub fn end(&self) -> f64 {
        self.start_of(self.len)
    }
}

/// How a Parent Line's level is drawn: Error and Fatal red, Warn orange, the
/// rest grey.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LevelClass {
    Other = 0,
    Warn = 1,
    Error = 2,
}

/// The class of a Continuation Line in a class list: not counted.
pub const CONTINUATION: u8 = 3;

/// A row's class as kept in a class list (one byte a row).
pub fn class_of(level: Option<LogLevel>, continues: bool) -> u8 {
    if continues {
        return CONTINUATION;
    }
    (match level {
        Some(LogLevel::Error | LogLevel::Fatal) => LevelClass::Error,
        Some(LogLevel::Warn) => LevelClass::Warn,
        _ => LevelClass::Other,
    }) as u8
}

/// The effective time of each row of a view (`NAN` when it has none):
/// read from the view's index row by row, with no list of them.
pub trait RowTimes {
    fn len(&self) -> usize;
    fn ts(&self, row: usize) -> f64;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The time of `row`, `None` past the last row.
    fn get(&self, row: usize) -> Option<f64> {
        (row < self.len()).then(|| self.ts(row))
    }
}

impl RowTimes for Vec<f64> {
    fn len(&self) -> usize {
        Vec::len(self)
    }

    fn ts(&self, row: usize) -> f64 {
        self[row]
    }
}

impl RowTimes for crate::filter_index::ViewIndexRead<'_> {
    fn len(&self) -> usize {
        crate::filter_index::ViewIndexRead::len(self)
    }

    fn ts(&self, row: usize) -> f64 {
        crate::filter_index::ViewIndexRead::ts(self, row)
    }
}

/// The per-row facts a count reads: each row's effective time (`NAN` when
/// it has none) and class ([`class_of`]).
#[derive(Clone, Copy)]
pub struct RowFacts<'a> {
    pub ts: &'a dyn RowTimes,
    pub class: &'a [u8],
}

impl RowFacts<'_> {
    pub fn len(&self) -> usize {
        self.ts.len().min(self.class.len())
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Counts by class: other, warn, error.
pub type ClassCounts = [u32; 3];

/// The counts of a histogram over a grid.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Buckets {
    /// The columns these counts are for.
    pub grid: Option<Grid>,
    /// Parent Lines in each column, by class.
    pub counts: Vec<ClassCounts>,
    /// Of those, the ones that pass the Filter; `None` with no Filter.
    pub filtered: Option<Vec<ClassCounts>>,
    /// Whether a column has a Match of the Search Expression.
    pub matched: Vec<bool>,
    /// Parent Lines with no time.
    pub untimed: usize,
    /// The first of them.
    pub first_untimed: Option<usize>,
}

/// Lines came with a time outside the axis: it has to grow and everything
/// be counted again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NeedsRecount;

impl Buckets {
    fn empty(grid: Grid, filtering: bool) -> Self {
        Self {
            grid: Some(grid),
            counts: vec![[0; 3]; grid.len],
            filtered: filtering.then(|| vec![[0; 3]; grid.len]),
            matched: vec![false; grid.len],
            untimed: 0,
            first_untimed: None,
        }
    }

    /// The tallest column.
    pub fn max_total(&self) -> u32 {
        self.counts.iter().map(|c| c.iter().sum::<u32>()).max().unwrap_or(0)
    }

    /// Count the rows `from..` of `rows`; `filter_pass` and `matches` hold
    /// rows from `from` on (ascending), and are walked alongside.
    fn count(
        &mut self,
        rows: RowFacts,
        from: usize,
        full: &Range<f64>,
        filter_pass: Option<crate::filter::PassFrom<'_>>,
        matches: &[usize],
    ) -> Result<(), NeedsRecount> {
        let Some(grid) = self.grid else { return Ok(()) };
        use crate::filter::PassFrom;
        // `None`: every row passes.
        let mut pass = filter_pass.map(|p| match p {
            PassFrom::All => None,
            PassFrom::Rows(rows) => Some(rows.iter().copied().peekable()),
        });
        let mut outside = false;
        for row in from..rows.len() {
            let class = rows.class[row];
            if class == CONTINUATION {
                continue;
            }
            let passes = match pass.as_mut() {
                Some(Some(p)) => {
                    while p.next_if(|&r| r < row).is_some() {}
                    p.next_if_eq(&row).is_some()
                }
                Some(None) => true,
                None => false,
            };
            let t = rows.ts.ts(row);
            if t.is_nan() {
                self.untimed += 1;
                self.first_untimed.get_or_insert(row);
                continue;
            }
            if t < full.start || t > full.end {
                outside = true;
            }
            let Some(col) = grid.column_of(t) else { continue };
            let class = usize::from(class.min(2));
            self.counts[col][class] += 1;
            if passes {
                if let Some(filtered) = self.filtered.as_mut() {
                    filtered[col][class] += 1;
                }
            }
        }
        // A Match marks its column, whether a Parent Line or not.
        for &row in matches {
            if let Some(col) = rows.ts.get(row).and_then(|t| grid.column_of(t)) {
                self.matched[col] = true;
            }
        }
        if outside { Err(NeedsRecount) } else { Ok(()) }
    }

    /// Count the rows `from` on that came after the last count: added to
    /// the columns, the rows before not counted again. `filter_pass` and
    /// `matches` are the passing rows and Matches from `from` on.
    /// `NeedsRecount` when one has a time outside `axis.full`.
    pub fn append(
        &mut self,
        rows: RowFacts,
        axis: &TimeAxis,
        from: usize,
        filter_pass: Option<crate::filter::PassFrom<'_>>,
        matches: &[usize],
    ) -> Result<(), NeedsRecount> {
        if self.grid != Some(axis.grid()) || filter_pass.is_some() != self.filtered.is_some() {
            return Err(NeedsRecount);
        }
        self.count(rows, from, &axis.full, filter_pass, matches)
    }
}

/// Count every row of `rows` on the columns of `axis`: Parent Lines by
/// class, those in `filter_pass` (ascending rows; `None` with no Filter)
/// apart, the columns holding one of `matches` (ascending rows), and the
/// rows with no time.
pub fn count_buckets(rows: RowFacts, axis: &TimeAxis, filter_pass: Option<crate::filter::PassFrom<'_>>, matches: &[usize]) -> Buckets {
    let mut buckets = Buckets::empty(axis.grid(), filter_pass.is_some());
    // The axis comes from these rows: none is outside it.
    let _ = buckets.count(rows, 0, &(f64::NEG_INFINITY..f64::INFINITY), filter_pass, matches);
    buckets
}

/// The earliest and latest time among `ts` (`NAN` skipped).
pub fn span_of(ts: impl IntoIterator<Item = f64>) -> Option<Range<f64>> {
    let mut span: Option<Range<f64>> = None;
    for t in ts.into_iter().filter(|t| !t.is_nan()) {
        span = Some(match span {
            None => t..t,
            Some(s) => s.start.min(t)..s.end.max(t),
        });
    }
    span
}

/// The shortest axis, in seconds: a view whose times all fall within it
/// (one line, or many written in the same instant) is shown on a minute
/// around them, not stretched across the strip.
pub const MIN_SPAN: f64 = 60.0;

/// `span` widened about its middle to [`MIN_SPAN`] when shorter.
pub fn pad_span(span: Range<f64>) -> Range<f64> {
    if span.end - span.start >= MIN_SPAN {
        return span;
    }
    let mid = (span.start + span.end) / 2.0;
    mid - MIN_SPAN / 2.0..mid + MIN_SPAN / 2.0
}

/// `full` grown to hold `t`, with room to spare so the next lines of a live
/// view fit without growing it again at once: a tenth of its length, or a
/// minute at least, rounded out to a whole step of that size.
pub fn grow_full(full: &Range<f64>, t: f64) -> Range<f64> {
    if t >= full.start && t <= full.end {
        return full.clone();
    }
    let room = ((full.end - full.start) * 0.1).max(60.0);
    let unit = STEPS.iter().rev().copied().find(|&s| s <= room).unwrap_or(STEPS[0]);
    if t > full.end {
        full.start..((t + room) / unit).ceil() * unit
    } else {
        ((t - room) / unit).floor() * unit..full.end
    }
}

/// The first row whose time is `time` or later. With `sorted` the times
/// ascend, those with none (`NAN`) last, and the row is found by halving;
/// else every row is looked at (a File View's times may go back).
pub fn first_row_at(ts: &dyn RowTimes, time: f64, sorted: bool) -> Option<usize> {
    if sorted {
        let timed = partition_point(ts.len(), |row| !ts.ts(row).is_nan());
        let at = partition_point(timed, |row| ts.ts(row) < time);
        (at < timed).then_some(at)
    } else {
        (0..ts.len()).find(|&row| ts.ts(row) >= time)
    }
}

/// The first of `0..len` for which `before` is false (`before` is true
/// for a run of rows, then false).
fn partition_point(len: usize, before: impl Fn(usize) -> bool) -> usize {
    let (mut lo, mut hi) = (0, len);
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if before(mid) { lo = mid + 1 } else { hi = mid }
    }
    lo
}

/// A time bound as the Time Range shows it: `HH:MM:SS` (with milliseconds
/// when the step is under a second) when `t` is on `anchor_day` (the day a
/// time of day is read on), else `YYYY-MM-DD HH:MM:SS`. Both are read back
/// by `parse_time_bound`.
pub fn format_time_bound(t: f64, anchor_day: Option<f64>, step: Step) -> String {
    let fraction = step.seconds() < 1.0;
    let Some(dt) = naive(t) else { return String::new() };
    let time = if fraction { dt.format("%H:%M:%S%.3f") } else { dt.format("%H:%M:%S") };
    match anchor_day {
        Some(day) if (t / 86_400.0).floor() * 86_400.0 == day => time.to_string(),
        _ => {
            let date = dt.format("%Y-%m-%d");
            format!("{date} {time}")
        }
    }
}

/// The From and To a Time Range from `from` to `to` shows: times of day when
/// both are on `anchor_day`, full dates and times otherwise.
pub fn format_time_range(from: f64, to: f64, anchor_day: f64, step: Step) -> (String, String) {
    let day = |t: f64| (t / 86_400.0).floor() * 86_400.0;
    let anchor = (day(from) == anchor_day && day(to) == anchor_day).then_some(anchor_day);
    (format_time_bound(from, anchor, step), format_time_bound(to, anchor, step))
}

fn naive(t: f64) -> Option<chrono::NaiveDateTime> {
    let secs = t.floor();
    let nanos = ((t - secs) * 1e9).round().min(999_999_999.0) as u32;
    chrono::DateTime::from_timestamp(secs as i64, nanos).map(|d| d.naive_utc())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::parse_time_bound;

    /// 2026-10-28 00:00:00 UTC.
    const DAY: f64 = 1_793_145_600.0;

    fn hm(h: u32, m: u32) -> f64 {
        DAY + f64::from(h * 3600 + m * 60)
    }

    #[test]
    fn a_span_of_one_instant_is_widened_to_a_minute() {
        let at = DAY + 0.312;
        assert_eq!(pad_span(at..at), at - 30.0..at + 30.0);
        assert_eq!(pad_span(DAY..DAY + 120.0), DAY..DAY + 120.0);
        let axis = TimeAxis::new(pad_span(at..at), 1600.0);
        // A thin column, not one across the strip.
        assert!(axis.grid().len > 1);
    }

    #[test]
    fn an_hour_on_720_pixels_is_30s_columns() {
        let visible = hm(10, 0)..hm(11, 0);
        assert_eq!(Step::for_width(&visible, 720.0), Step(30.0));
        // Columns about 4–6 px wide.
        let px = 720.0 / (3600.0 / 30.0);
        assert!((4.0..=6.0).contains(&px));
        assert_eq!(Step::for_width(&(0.0..10.0 * 86_400.0), 100.0), Step(86_400.0));
        assert_eq!(Step::for_width(&(0.0..10.0), 720.0), Step(1.0));
    }

    fn facts(rows: &[(f64, Option<LogLevel>, bool)]) -> (Vec<f64>, Vec<u8>) {
        (rows.iter().map(|r| r.0).collect(), rows.iter().map(|r| class_of(r.1, r.2)).collect())
    }

    fn axis(from: f64, to: f64) -> TimeAxis {
        TimeAxis { full: from..to, visible: from..to, step: Step(60.0) }
    }

    #[test]
    fn counts_parent_lines_by_level_not_continuations() {
        let mut rows = vec![(hm(10, 2), Some(LogLevel::Error), false)];
        // An exception of 80 lines under the ERROR line.
        rows.extend((0..80).map(|_| (hm(10, 2), None, true)));
        rows.push((hm(10, 2), Some(LogLevel::Warn), false));
        rows.push((hm(10, 3), Some(LogLevel::Fatal), false));
        rows.push((hm(10, 3), Some(LogLevel::Info), false));
        let (ts, class) = facts(&rows);
        let b = count_buckets(RowFacts { ts: &ts, class: &class }, &axis(hm(10, 0), hm(10, 5)), None, &[]);
        assert_eq!(b.counts[2], [0, 1, 1]);
        assert_eq!(b.counts[3], [1, 0, 1]);
        assert_eq!(b.filtered, None);
    }

    #[test]
    fn lines_with_no_level_are_grey() {
        let (ts, class) = facts(&[(hm(10, 0), None, false), (hm(10, 1), None, false)]);
        let b = count_buckets(RowFacts { ts: &ts, class: &class }, &axis(hm(10, 0), hm(10, 1)), None, &[]);
        assert!(b.counts.iter().all(|c| c[1] == 0 && c[2] == 0));
        assert_eq!(b.counts.iter().map(|c| c[0]).sum::<u32>(), 2);
    }

    #[test]
    fn filter_and_matches_are_counted_alongside() {
        let rows = [
            (hm(10, 30), Some(LogLevel::Info), false),
            (hm(10, 31), Some(LogLevel::Error), false),
            (hm(10, 31), None, true),
            (hm(10, 32), Some(LogLevel::Info), false),
            // Nothing from 10:41 to 10:44.
            (hm(10, 45), Some(LogLevel::Error), false),
        ];
        let (ts, class) = facts(&rows);
        let r = RowFacts { ts: &ts, class: &class };
        let a = axis(hm(10, 30), hm(10, 45));
        // Filter: level = ERROR. Matches of `timeout`: rows 1 and 2.
        let b = count_buckets(r, &a, Some(crate::filter::PassFrom::Rows(&[1, 4])), &[1, 2]);
        let filtered = b.filtered.as_ref().unwrap();
        assert_eq!((b.counts[1], filtered[1]), ([0, 0, 1], [0, 0, 1]));
        assert_eq!((b.counts[0], filtered[0]), ([1, 0, 0], [0, 0, 0]));
        assert_eq!(filtered[15], [0, 0, 1]);
        for col in 11..15 {
            assert_eq!((b.counts[col], filtered[col]), ([0; 3], [0; 3]), "{col}");
        }
        let marked: Vec<usize> = (0..b.matched.len()).filter(|&c| b.matched[c]).collect();
        assert_eq!(marked, vec![1]);
    }

    #[test]
    fn rows_with_no_time_are_counted_apart() {
        let (ts, class) = facts(&[
            (hm(10, 0), Some(LogLevel::Info), false),
            (f64::NAN, None, false),
            (f64::NAN, None, true),
            (f64::NAN, None, false),
        ]);
        let b = count_buckets(RowFacts { ts: &ts, class: &class }, &axis(hm(10, 0), hm(10, 1)), None, &[]);
        assert_eq!((b.untimed, b.first_untimed), (2, Some(1)));
        assert_eq!(b.counts.iter().map(|c| c.iter().sum::<u32>()).sum::<u32>(), 1);
    }

    #[test]
    fn appended_rows_add_up_without_counting_again() {
        let mut rows = vec![(hm(10, 0), Some(LogLevel::Info), false)];
        let (ts, class) = facts(&rows);
        let a = axis(hm(10, 0), hm(10, 5));
        let mut b = count_buckets(RowFacts { ts: &ts, class: &class }, &a, None, &[]);
        // 200 lines in the last column.
        rows.extend((0..200).map(|i| (hm(10, 5), Some(if i % 2 == 0 { LogLevel::Warn } else { LogLevel::Info }), false)));
        let (ts, class) = facts(&rows);
        assert_eq!(b.append(RowFacts { ts: &ts, class: &class }, &a, 1, None, &[]), Ok(()));
        assert_eq!(b.counts[5], [100, 100, 0]);
        assert_eq!(b.counts[0], [1, 0, 0]);
        // A line past the end of the axis.
        rows.push((hm(10, 9), None, false));
        let (ts, class) = facts(&rows);
        assert_eq!(b.append(RowFacts { ts: &ts, class: &class }, &a, 201, None, &[]), Err(NeedsRecount));
        // Counted on another grid: count again.
        let zoomed = TimeAxis { visible: hm(10, 0)..hm(10, 2), ..a.clone() };
        assert_eq!(b.append(RowFacts { ts: &ts, class: &class }, &zoomed, 202, None, &[]), Err(NeedsRecount));
    }

    #[test]
    fn the_axis_grows_with_room_rounded_out() {
        let full = hm(10, 0)..hm(11, 0);
        assert_eq!(grow_full(&full, hm(10, 30)), full);
        // A tenth of an hour is 6 minutes: rounded to 5-minute steps.
        let grown = grow_full(&full, hm(11, 1));
        assert_eq!(grown, hm(10, 0)..hm(11, 10));
        assert!(grow_full(&grown, hm(11, 9)) == grown, "the next lines fit");
        assert_eq!(grow_full(&full, hm(9, 58)), hm(9, 50)..hm(11, 0));
        // A short axis grows by a minute at least.
        let short = hm(10, 0)..hm(10, 0);
        assert_eq!(grow_full(&short, hm(10, 0) + 1.0), hm(10, 0)..hm(10, 2));
    }

    #[test]
    fn first_row_at_a_time() {
        let sorted = vec![hm(10, 0), hm(10, 0), hm(10, 2), hm(10, 5), f64::NAN];
        assert_eq!(first_row_at(&sorted, hm(10, 0), true), Some(0));
        assert_eq!(first_row_at(&sorted, hm(10, 1), true), Some(2), "an empty column");
        assert_eq!(first_row_at(&sorted, hm(10, 6), true), None);
        // Times that go back: looked at in order.
        let unsorted = vec![f64::NAN, hm(10, 5), hm(10, 1), hm(10, 3)];
        assert_eq!(first_row_at(&unsorted, hm(10, 2), false), Some(1));
        assert_eq!(first_row_at(&unsorted, hm(10, 6), false), None);
    }

    #[test]
    fn time_bounds_read_back_as_written() {
        let step = Step(60.0);
        let (from, to) = format_time_range(hm(10, 30), hm(10, 35), DAY, step);
        assert_eq!((from.as_str(), to.as_str()), ("10:30:00", "10:35:00"));
        assert_eq!(parse_time_bound(&from, DAY), Ok(Some(hm(10, 30))));
        // Across midnight: dates in full.
        let (from, to) = format_time_range(hm(23, 50), hm(24, 10), DAY, step);
        assert_eq!((from.as_str(), to.as_str()), ("2026-10-28 23:50:00", "2026-10-29 00:10:00"));
        assert_eq!(parse_time_bound(&to, DAY), Ok(Some(hm(24, 10))));
        // Another day than the anchor's.
        let (from, _) = format_time_range(hm(34, 0), hm(35, 0), DAY, step);
        assert_eq!(parse_time_bound(&from, DAY), Ok(Some(hm(34, 0))));
        // Under a second: milliseconds kept.
        let t = hm(10, 30) + 1.25;
        let text = format_time_bound(t, Some(DAY), Step(0.5));
        assert_eq!(text, "10:30:01.250");
        assert_eq!(parse_time_bound(&text, DAY), Ok(Some(t)));
    }

    #[test]
    fn zoom_stays_within_the_axis() {
        let mut a = TimeAxis::new(hm(10, 0)..hm(11, 0), 720.0);
        a.zoom(hm(10, 30), 0.5, 720.0);
        assert_eq!(a.visible, hm(10, 15)..hm(10, 45));
        assert_eq!(a.step, Step(10.0));
        a.zoom(hm(10, 15), 100.0, 720.0);
        assert!(a.shows_all());
        a.set_visible(hm(9, 0)..hm(9, 30), 720.0);
        assert_eq!(a.visible, hm(10, 0)..hm(10, 30));
    }
}
