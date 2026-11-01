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

impl LevelClass {
    /// The class of a byte of a class list (a Continuation Line's aside).
    pub fn from_u8(class: u8) -> LevelClass {
        match class {
            2 => LevelClass::Error,
            1 => LevelClass::Warn,
            _ => LevelClass::Other,
        }
    }
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

/// The most lanes drawn; the other values share the "Other" lane.
pub const MAX_LANES: usize = 8;

/// The value id of a row with no value.
use crate::filter_index::MISSING as NO_VALUE;

/// The value id of each row in the column the lanes split by (its Filter
/// value ids, `MISSING` for none), read row by row with no list of them.
pub trait RowValueIds {
    /// The id of `row`, `None` when it is not known.
    fn id(&self, row: usize) -> Option<u16>;
}

impl RowValueIds for Vec<u16> {
    fn id(&self, row: usize) -> Option<u16> {
        self.get(row).copied()
    }
}

impl RowValueIds for crate::filter_index::RowIds<'_> {
    fn id(&self, row: usize) -> Option<u16> {
        self.id_of_row(row)
    }
}

/// The value of each row in the column the lanes split by, and how many
/// values the column has.
#[derive(Clone, Copy)]
pub struct LaneIds<'a> {
    pub ids: &'a dyn RowValueIds,
    pub values: usize,
}

/// Which values have a lane of their own (openspec change `add-swimlane`,
/// decision 3): the [`MAX_LANES`] with the most Parent Lines, most first
/// (a tie by value order); the rest share "Other", and the lines with no
/// value have "(empty)".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LaneSet {
    /// The value id of each lane of its own, in lane order.
    pub slots: Vec<u16>,
    /// How many values share the "Other" lane.
    pub other_count: usize,
    /// Some Parent Line has no value: the "(empty)" lane is drawn.
    pub has_empty: bool,
}

impl LaneSet {
    /// Choose the lanes from `total`: the Parent Lines of each value id,
    /// and last of all those with no value.
    pub fn choose(total: &[usize]) -> LaneSet {
        let Some((&empty, values)) = total.split_last() else { return LaneSet::default() };
        let mut ranked: Vec<usize> = (0..values.len()).filter(|&v| values[v] > 0).collect();
        // Most lines first; the sort is stable, so a tie keeps value order.
        ranked.sort_by(|&a, &b| values[b].cmp(&values[a]));
        let slots: Vec<u16> = ranked.iter().take(MAX_LANES).map(|&v| v as u16).collect();
        LaneSet { other_count: ranked.len() - slots.len(), slots, has_empty: empty > 0 }
    }
}

/// One lane among a [`LaneSet`]'s.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lane {
    /// The lane of this value id.
    Value(u16),
    /// The lines with no value.
    Empty,
    /// The values with no lane of their own.
    Other,
}

/// Lane of a value in `Lanes::lane_of`: not seen yet, or the Other lane.
const UNSEEN: u8 = u8::MAX;
const IN_OTHER: u8 = u8::MAX - 1;

/// The lanes of a histogram: for each lane and column, the highest class
/// of its Parent Lines there, kept as a byte: the class plus one, 0 for
/// none. Read it with [`class_at`](Lanes::class_at) or
/// [`classes`](Lanes::classes).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Lanes {
    pub set: LaneSet,
    /// One row of columns per value of `set.slots`.
    pub slot_level: Vec<Vec<u8>>,
    /// The "(empty)" lane, while `set.has_empty`.
    pub empty_level: Vec<u8>,
    /// The "Other" lane, while `set.other_count > 0`.
    pub other_level: Vec<u8>,
    /// The lane each value id is in: an index into `slots`, [`IN_OTHER`],
    /// or [`UNSEEN`].
    lane_of: Vec<u8>,
}

impl Lanes {
    /// The lanes drawn, in order: the values', "(empty)", then "Other".
    pub fn order(&self) -> Vec<Lane> {
        let mut lanes: Vec<Lane> = self.set.slots.iter().map(|&v| Lane::Value(v)).collect();
        if self.set.has_empty {
            lanes.push(Lane::Empty);
        }
        if self.set.other_count > 0 {
            lanes.push(Lane::Other);
        }
        lanes
    }

    /// The highest class of `lane`'s Parent Lines in each column, `None`
    /// where it has none.
    pub fn classes(&self, lane: Lane) -> impl Iterator<Item = Option<LevelClass>> + '_ {
        self.level(lane).iter().map(|&l| (l > 0).then(|| LevelClass::from_u8(l - 1)))
    }

    /// The highest class of `lane`'s Parent Lines in column `col`.
    pub fn class_at(&self, lane: Lane, col: usize) -> Option<LevelClass> {
        self.level(lane).get(col).and_then(|&l| (l > 0).then(|| LevelClass::from_u8(l - 1)))
    }

    /// The columns of `lane`, as kept.
    fn level(&self, lane: Lane) -> &[u8] {
        match lane {
            Lane::Value(v) => self.set.slots.iter().position(|&s| s == v).map_or(&[], |i| &self.slot_level[i]),
            Lane::Empty => &self.empty_level,
            Lane::Other => &self.other_level,
        }
    }

    /// Whether a row of value `id` belongs in `lane`.
    pub fn holds(&self, lane: Lane, id: u16) -> bool {
        match lane {
            Lane::Value(v) => id == v,
            Lane::Empty => id == NO_VALUE,
            Lane::Other => id != NO_VALUE && !self.set.slots.contains(&id),
        }
    }

    /// The lanes chosen from a whole count: `flat` holds the level of each
    /// value (the no-value lines last) on each of `len` columns, `total`
    /// the Parent Lines of each.
    fn from_flat(flat: &[u8], total: &[usize], len: usize) -> Lanes {
        let set = LaneSet::choose(total);
        let values = total.len() - 1;
        let row = |v: usize| flat[v * len..(v + 1) * len].to_vec();
        let mut lane_of = vec![UNSEEN; values];
        for (i, &v) in set.slots.iter().enumerate() {
            lane_of[v as usize] = i as u8;
        }
        let mut other_level = vec![0u8; if set.other_count > 0 { len } else { 0 }];
        for v in 0..values {
            if total[v] == 0 || lane_of[v] != UNSEEN {
                continue;
            }
            lane_of[v] = IN_OTHER;
            for (o, &l) in other_level.iter_mut().zip(&flat[v * len..(v + 1) * len]) {
                *o = (*o).max(l);
            }
        }
        Lanes {
            slot_level: set.slots.iter().map(|&v| row(v as usize)).collect(),
            empty_level: if set.has_empty { row(values) } else { Vec::new() },
            other_level,
            set,
            lane_of,
        }
    }

    /// Add a Parent Line of value `id` and class `class`, in column `col`
    /// when it has one, after the lanes were chosen: a value without a lane
    /// takes a free one, else it goes in "Other". The lanes there keep
    /// their order.
    fn add(&mut self, id: u16, col: Option<usize>, class: u8, len: usize) {
        let lane = if id == NO_VALUE {
            if !self.set.has_empty {
                self.set.has_empty = true;
                self.empty_level = vec![0; len];
            }
            &mut self.empty_level
        } else {
            let v = id as usize;
            if v >= self.lane_of.len() {
                self.lane_of.resize(v + 1, UNSEEN);
            }
            if self.lane_of[v] == UNSEEN {
                if self.set.slots.len() < MAX_LANES {
                    self.lane_of[v] = self.set.slots.len() as u8;
                    self.set.slots.push(id);
                    self.slot_level.push(vec![0; len]);
                } else {
                    self.lane_of[v] = IN_OTHER;
                    if self.set.other_count == 0 {
                        self.other_level = vec![0; len];
                    }
                    self.set.other_count += 1;
                }
            }
            match self.lane_of[v] {
                IN_OTHER => &mut self.other_level,
                i => &mut self.slot_level[i as usize],
            }
        };
        if let Some(cell) = col.and_then(|c| lane.get_mut(c)) {
            *cell = (*cell).max(class + 1);
        }
    }
}

/// What a count walks alongside the rows: the rows passing the Filter
/// (`None` with no Filter) and the Matches, ascending, and the value of
/// each row when the lanes are counted.
#[derive(Clone, Copy)]
struct Alongside<'a> {
    filter_pass: Option<crate::filter::PassFrom<'a>>,
    matches: &'a [usize],
    lanes: Option<LaneIds<'a>>,
}

/// While counting everything: each value's level on each column (flat, a
/// row of columns per value, the no-value lines last) and its lines.
struct LaneTally {
    flat: Vec<u8>,
    total: Vec<usize>,
}

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
    /// The lanes, when the view is split by a column.
    pub lanes: Option<Lanes>,
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
            lanes: None,
        }
    }

    /// The tallest column.
    pub fn max_total(&self) -> u32 {
        self.counts.iter().map(|c| c.iter().sum::<u32>()).max().unwrap_or(0)
    }

    /// Count the rows `from..` of `rows`; what `along` holds is from `from`
    /// on. With lanes, each Parent Line is also put in its value's lane:
    /// into `tally` when given (a whole count), else into `self.lanes`.
    fn count(
        &mut self,
        rows: RowFacts,
        from: usize,
        full: &Range<f64>,
        along: Alongside,
        mut tally: Option<&mut LaneTally>,
    ) -> Result<(), NeedsRecount> {
        let Alongside { filter_pass, matches, lanes } = along;
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
            if let Some(lanes) = lanes {
                let id = lanes.ids.id(row).unwrap_or(NO_VALUE);
                let col = if t.is_nan() { None } else { grid.column_of(t) };
                match tally.as_deref_mut() {
                    Some(tally) => {
                        let v = if id == NO_VALUE { lanes.values } else { usize::from(id).min(lanes.values) };
                        tally.total[v] += 1;
                        if let Some(col) = col {
                            let cell = &mut tally.flat[v * grid.len + col];
                            *cell = (*cell).max(class.min(2) + 1);
                        }
                    }
                    None => {
                        if let Some(l) = self.lanes.as_mut() {
                            l.add(id, col, class.min(2), grid.len);
                        }
                    }
                }
            }
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
    /// The lanes keep the values they have (decision 4): a new value takes
    /// a free lane or goes in "Other".
    pub fn append(
        &mut self,
        rows: RowFacts,
        axis: &TimeAxis,
        from: usize,
        filter_pass: Option<crate::filter::PassFrom<'_>>,
        matches: &[usize],
        lanes: Option<LaneIds>,
    ) -> Result<(), NeedsRecount> {
        if self.grid != Some(axis.grid())
            || filter_pass.is_some() != self.filtered.is_some()
            || lanes.is_some() != self.lanes.is_some()
        {
            return Err(NeedsRecount);
        }
        self.count(rows, from, &axis.full, Alongside { filter_pass, matches, lanes }, None)
    }
}

/// Count every row of `rows` on the columns of `axis`: Parent Lines by
/// class, those in `filter_pass` (ascending rows; `None` with no Filter)
/// apart, the columns holding one of `matches` (ascending rows), and the
/// rows with no time. With `lanes`, also the lanes of the column's values,
/// in the same pass; they count every row, whatever the Filter.
pub fn count_buckets(
    rows: RowFacts,
    axis: &TimeAxis,
    filter_pass: Option<crate::filter::PassFrom<'_>>,
    matches: &[usize],
    lanes: Option<LaneIds>,
) -> Buckets {
    let grid = axis.grid();
    let mut buckets = Buckets::empty(grid, filter_pass.is_some());
    let mut tally = lanes.map(|l| LaneTally { flat: vec![0; (l.values + 1) * grid.len], total: vec![0; l.values + 1] });
    // The axis comes from these rows: none is outside it.
    let along = Alongside { filter_pass, matches, lanes };
    let _ = buckets.count(rows, 0, &(f64::NEG_INFINITY..f64::INFINITY), along, tally.as_mut());
    buckets.lanes = tally.map(|t| Lanes::from_flat(&t.flat, &t.total, grid.len));
    buckets
}

/// What the Swimlanes of a view are split by (openspec change
/// `add-swimlane`, decision 5). Kept with the view's Filter.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LaneBy {
    /// "No split": no lanes.
    None,
    /// The Filterable Column of this key.
    Column(String),
    /// Never chosen: the Log Name Column when it has two values or more,
    /// else no lanes.
    #[default]
    Default,
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
        let b = count_buckets(RowFacts { ts: &ts, class: &class }, &axis(hm(10, 0), hm(10, 5)), None, &[], None);
        assert_eq!(b.counts[2], [0, 1, 1]);
        assert_eq!(b.counts[3], [1, 0, 1]);
        assert_eq!(b.filtered, None);
    }

    #[test]
    fn lines_with_no_level_are_grey() {
        let (ts, class) = facts(&[(hm(10, 0), None, false), (hm(10, 1), None, false)]);
        let b = count_buckets(RowFacts { ts: &ts, class: &class }, &axis(hm(10, 0), hm(10, 1)), None, &[], None);
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
        let b = count_buckets(r, &a, Some(crate::filter::PassFrom::Rows(&[1, 4])), &[1, 2], None);
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
        let b = count_buckets(RowFacts { ts: &ts, class: &class }, &axis(hm(10, 0), hm(10, 1)), None, &[], None);
        assert_eq!((b.untimed, b.first_untimed), (2, Some(1)));
        assert_eq!(b.counts.iter().map(|c| c.iter().sum::<u32>()).sum::<u32>(), 1);
    }

    #[test]
    fn appended_rows_add_up_without_counting_again() {
        let mut rows = vec![(hm(10, 0), Some(LogLevel::Info), false)];
        let (ts, class) = facts(&rows);
        let a = axis(hm(10, 0), hm(10, 5));
        let mut b = count_buckets(RowFacts { ts: &ts, class: &class }, &a, None, &[], None);
        // 200 lines in the last column.
        rows.extend((0..200).map(|i| (hm(10, 5), Some(if i % 2 == 0 { LogLevel::Warn } else { LogLevel::Info }), false)));
        let (ts, class) = facts(&rows);
        assert_eq!(b.append(RowFacts { ts: &ts, class: &class }, &a, 1, None, &[], None), Ok(()));
        assert_eq!(b.counts[5], [100, 100, 0]);
        assert_eq!(b.counts[0], [1, 0, 0]);
        // A line past the end of the axis.
        rows.push((hm(10, 9), None, false));
        let (ts, class) = facts(&rows);
        assert_eq!(b.append(RowFacts { ts: &ts, class: &class }, &a, 201, None, &[], None), Err(NeedsRecount));
        // Counted on another grid: count again.
        let zoomed = TimeAxis { visible: hm(10, 0)..hm(10, 2), ..a.clone() };
        assert_eq!(b.append(RowFacts { ts: &ts, class: &class }, &zoomed, 202, None, &[], None), Err(NeedsRecount));
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

    /// Rows of (time, level, continues, value id).
    fn lane_rows(rows: &[(f64, Option<LogLevel>, bool, u16)]) -> (Vec<f64>, Vec<u8>, Vec<u16>) {
        let (ts, class) = facts(&rows.iter().map(|r| (r.0, r.1, r.2)).collect::<Vec<_>>());
        (ts, class, rows.iter().map(|r| r.3).collect())
    }

    #[test]
    fn two_services_err_in_their_own_lanes() {
        // service: db = 0, api = 1.
        let (ts, class, ids) = lane_rows(&[
            (hm(10, 30), Some(LogLevel::Error), false, 0),
            (hm(10, 30), Some(LogLevel::Info), false, 1),
            (hm(10, 31), Some(LogLevel::Error), false, 1),
            (hm(10, 31), Some(LogLevel::Warn), false, 0),
            (hm(10, 31), Some(LogLevel::Info), false, 0),
        ]);
        let lanes = Some(LaneIds { ids: &ids, values: 2 });
        let b = count_buckets(RowFacts { ts: &ts, class: &class }, &axis(hm(10, 30), hm(10, 32)), None, &[], lanes);
        let l = b.lanes.unwrap();
        assert_eq!(l.order(), [Lane::Value(0), Lane::Value(1)], "more lines first");
        // Highest class + 1: Error 3, Warn 2, the rest 1.
        assert_eq!(&l.level(Lane::Value(0))[..2], [3, 2]);
        assert_eq!(&l.level(Lane::Value(1))[..2], [1, 3]);
    }

    #[test]
    fn many_threads_keep_eight_lanes_and_other() {
        // 300 threads; thread v has 300 - v lines, so 0..8 have the most.
        let mut rows = Vec::new();
        for v in 0..300u16 {
            rows.extend((0..300 - v).map(|_| (hm(10, 0), Some(LogLevel::Info), false, v)));
        }
        let (ts, class, ids) = lane_rows(&rows);
        let lanes = Some(LaneIds { ids: &ids, values: 300 });
        let b = count_buckets(RowFacts { ts: &ts, class: &class }, &axis(hm(10, 0), hm(10, 1)), None, &[], lanes);
        let l = b.lanes.unwrap();
        assert_eq!(l.set.slots, (0..8).collect::<Vec<u16>>());
        assert_eq!(l.set.other_count, 292);
        assert!(!l.set.has_empty);
        assert_eq!(l.order().last(), Some(&Lane::Other));
        assert_eq!(l.level(Lane::Other)[0], 1);
    }

    #[test]
    fn a_tie_keeps_value_order() {
        assert_eq!(LaneSet::choose(&[2, 5, 5, 2, 0]).slots, [1, 2, 0, 3]);
    }

    #[test]
    fn lines_with_no_value_have_the_empty_lane_and_continuations_follow_their_parent() {
        let mut rows = vec![(hm(10, 0), Some(LogLevel::Error), false, 0)];
        // A stack trace of 40 lines under the `service=db` line.
        rows.extend((0..40).map(|_| (hm(10, 0), None, true, NO_VALUE)));
        let (ts, class, ids) = lane_rows(&rows);
        let lanes = Some(LaneIds { ids: &ids, values: 1 });
        let b = count_buckets(RowFacts { ts: &ts, class: &class }, &axis(hm(10, 0), hm(10, 1)), None, &[], lanes);
        assert_eq!(b.lanes.as_ref().unwrap().order(), [Lane::Value(0)], "no (empty) lane for continuations");

        // 12 Parent Lines with no service.
        rows.extend((0..12).map(|_| (hm(10, 1), Some(LogLevel::Info), false, NO_VALUE)));
        let (ts, class, ids) = lane_rows(&rows);
        let lanes = Some(LaneIds { ids: &ids, values: 1 });
        let b = count_buckets(RowFacts { ts: &ts, class: &class }, &axis(hm(10, 0), hm(10, 1)), None, &[], lanes);
        let l = b.lanes.unwrap();
        assert_eq!(l.order(), [Lane::Value(0), Lane::Empty]);
        assert_eq!(&l.level(Lane::Empty)[..2], [0, 1]);
    }

    #[test]
    fn a_new_value_while_tailing_takes_a_free_lane_and_the_lanes_keep_their_order() {
        let mut rows: Vec<_> = (0..3u16).flat_map(|v| (0..3 - v).map(move |_| (hm(10, 0), Some(LogLevel::Info), false, v))).collect();
        let (ts, class, ids) = lane_rows(&rows);
        let a = axis(hm(10, 0), hm(10, 5));
        let mut b = count_buckets(RowFacts { ts: &ts, class: &class }, &a, None, &[], Some(LaneIds { ids: &ids, values: 3 }));
        let before = b.lanes.as_ref().unwrap().order();
        // `auth` (value 3) comes, more than any other.
        let from = rows.len();
        rows.extend((0..10).map(|_| (hm(10, 2), Some(LogLevel::Error), false, 3)));
        let (ts, class, ids) = lane_rows(&rows);
        let appended = b.append(RowFacts { ts: &ts, class: &class }, &a, from, None, &[], Some(LaneIds { ids: &ids, values: 4 }));
        assert_eq!(appended, Ok(()));
        let l = b.lanes.as_ref().unwrap();
        assert_eq!(l.order()[..3], before[..], "the lanes there do not move");
        assert_eq!(l.order()[3], Lane::Value(3));
        assert_eq!(l.level(Lane::Value(3))[2], 3);
    }

    #[test]
    fn a_new_value_goes_in_other_when_the_lanes_are_full() {
        let mut rows: Vec<_> = (0..8u16).map(|v| (hm(10, 0), Some(LogLevel::Info), false, v)).collect();
        let (ts, class, ids) = lane_rows(&rows);
        let a = axis(hm(10, 0), hm(10, 5));
        let mut b = count_buckets(RowFacts { ts: &ts, class: &class }, &a, None, &[], Some(LaneIds { ids: &ids, values: 8 }));
        rows.push((hm(10, 4), Some(LogLevel::Warn), false, 8));
        let (ts, class, ids) = lane_rows(&rows);
        b.append(RowFacts { ts: &ts, class: &class }, &a, 8, None, &[], Some(LaneIds { ids: &ids, values: 9 })).unwrap();
        let l = b.lanes.as_ref().unwrap();
        assert_eq!(l.set.slots.len(), 8);
        assert_eq!((l.set.other_count, l.level(Lane::Other)[4]), (1, 2));
    }

    #[test]
    fn the_filter_does_not_take_lines_out_of_the_lanes() {
        let (ts, class, ids) = lane_rows(&[
            (hm(10, 0), Some(LogLevel::Info), false, 0),
            (hm(10, 1), Some(LogLevel::Error), false, 1),
            (hm(10, 2), Some(LogLevel::Info), false, 2),
        ]);
        // Only `db` (value 0) passes the Filter.
        let lanes = Some(LaneIds { ids: &ids, values: 3 });
        let b = count_buckets(RowFacts { ts: &ts, class: &class }, &axis(hm(10, 0), hm(10, 3)), Some(crate::filter::PassFrom::Rows(&[0])), &[], lanes);
        let l = b.lanes.unwrap();
        assert_eq!(l.order().len(), 3);
        assert_eq!((l.level(Lane::Value(1))[1], l.level(Lane::Value(2))[2]), (3, 1));
    }
}
