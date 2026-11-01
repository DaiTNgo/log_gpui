//! Logic: the Swimlanes under the Time Histogram (openspec change
//! `add-swimlane`): which column they split by (Lane By, kept with the
//! view's Filter), what the "Lane by" menu offers, and what clicking a lane
//! does. The lanes are counted with the histogram (`time_histogram`).

use super::AppView;
use crate::columns::{LOG_NAME_KEY, SOURCE_KEY};
use crate::filter_index::{RowIds, ViewIndexRead};
use crate::model::LogLevel;
use crate::time_histogram::{self as th, Lane, LaneBy};
use crate::workspace::working_set::TabTarget;
use gpui::Context;

/// Field Columns the menu suggests, after MODULE, first found first.
const SUGGESTED_FIELDS: [&str; 6] = ["service", "module", "component", "thread", "host", "pod"];

/// What the pointer is over on the Swimlanes: a lane in a column, its
/// Parent Lines there and the highest level among them.
#[derive(Debug, Clone, PartialEq)]
pub struct LaneHover {
    pub lane: Lane,
    pub col: usize,
    pub lines: usize,
    pub level: Option<LogLevel>,
}

/// How severe a level is, for the highest of a column: a line whose level
/// is not known is the least.
fn severity(level: LogLevel) -> i8 {
    if level == LogLevel::Unknown { -1 } else { level as i8 }
}

/// The value id of each row in the column `key` of the view's index, and
/// how many values it has, when it keeps them: what the lanes are counted
/// from.
pub(crate) fn lane_ids<'a>(index: &'a ViewIndexRead, key: &str) -> Option<(RowIds<'a>, usize)> {
    Some((index.ids(key)?, index.column(key)?.values.len()))
}

/// A column in the "Lane by" menu.
#[derive(Debug, Clone, PartialEq)]
pub struct LaneChoice {
    pub key: String,
    pub label: String,
    /// Why it cannot be chosen, when it cannot.
    pub unavailable: Option<String>,
}

impl AppView {
    /// What the open view's lanes are split by, as chosen (or never).
    pub fn lane_by(&self) -> LaneBy {
        let Some(space) = self.active_space() else { return LaneBy::None };
        match self.active_target() {
            Some(TabTarget::LogSpace { .. }) => space.lane_by.clone(),
            Some(TabTarget::FileView { file_id, .. }) => space.file_lane_by.get(&file_id).cloned().unwrap_or_default(),
            _ => LaneBy::None,
        }
    }

    /// The column the lanes split by now, when there are lanes: `Default`
    /// is the Log Name Column of a Log Space with two values or more, and a
    /// chosen column the lanes cannot split by (gone from the view, past
    /// the limit of values, or still read) falls back to it (decision 5).
    pub fn lane_column(&self) -> Option<String> {
        let index = self.filter_index.read(&self.view.rows);
        let chosen = match self.lane_by() {
            LaneBy::None => return None,
            LaneBy::Column(key) => Some(key).filter(|k| index.ids(k).is_some()),
            LaneBy::Default => None,
        };
        chosen.or_else(|| {
            let two = index.ids(LOG_NAME_KEY).is_some() && index.column(LOG_NAME_KEY)?.values.len() >= 2;
            (self.active_is_log_space() && two).then(|| LOG_NAME_KEY.to_string())
        })
    }

    /// The column the menu puts first as a suggestion while there are no
    /// lanes: MODULE, else the first Field Column named like a service;
    /// only one the menu lets the user choose.
    pub fn lane_suggestion(&self) -> Option<String> {
        if self.lane_column().is_some() {
            return None;
        }
        let index = self.filter_index.read(&self.view.rows);
        let choosable: Vec<_> = index.columns_in_use().filter(|c| index.ids(&c.key).is_some()).collect();
        if choosable.iter().any(|c| c.key == "module") {
            return Some("module".to_string());
        }
        SUGGESTED_FIELDS.iter().find_map(|name| {
            choosable.iter().find(|c| c.field().is_some() && c.key.eq_ignore_ascii_case(name)).map(|c| c.key.clone())
        })
    }

    /// The columns of the "Lane by" menu: every Filterable Column of the
    /// view, and those that cannot be chosen with the reason.
    pub fn lane_choices(&self) -> Vec<LaneChoice> {
        let suggested = self.lane_suggestion();
        let index = self.filter_index.read(&self.view.rows);
        let mut choices: Vec<LaneChoice> = index
            .columns_in_use()
            .map(|c| LaneChoice {
                key: c.key.clone(),
                label: c.label.clone(),
                unavailable: if c.overflow {
                    Some(format!("Too many values (> {})", crate::filter_index::MAX_DISTINCT))
                } else if index.ids(&c.key).is_none() {
                    Some("Still reading".to_string())
                } else {
                    None
                },
            })
            .collect();
        if let Some(at) = suggested.and_then(|k| choices.iter().position(|c| c.key == k)) {
            let first = choices.remove(at);
            choices.insert(0, first);
        }
        choices
    }

    /// Split the open view's lanes by `lane_by`, kept with its Filter.
    pub fn set_lane_by(&mut self, lane_by: LaneBy, cx: &mut Context<Self>) {
        let target = self.active_target();
        let Some(space_id) = target.as_ref().and_then(|t| t.space_id()).map(str::to_string) else { return };
        let Some(space) = self.active_project_mut().and_then(|p| p.log_space_mut(&space_id)) else { return };
        match target {
            Some(TabTarget::LogSpace { .. }) => space.lane_by = lane_by,
            Some(TabTarget::FileView { file_id, .. }) => {
                space.file_lane_by.insert(file_id, lane_by);
            }
            _ => return,
        }
        self.schedule_session_save(cx);
        self.refresh_histogram(cx);
        cx.notify();
    }

    /// How many values share the "Other" lane.
    fn other_count(&self) -> usize {
        self.histogram.shown.lanes.as_ref().map_or(0, |l| l.set.other_count)
    }

    /// The name a lane shows.
    pub fn lane_name(&self, lane: Lane) -> String {
        match lane {
            Lane::Value(v) => {
                let index = self.filter_index.read(&self.view.rows);
                self.histogram
                    .lanes_for
                    .as_deref()
                    .and_then(|key| index.column(key))
                    .and_then(|c| c.values.get(usize::from(v)))
                    .map_or_else(String::new, |v| v.display.clone())
            }
            Lane::Empty => "(empty)".to_string(),
            Lane::Other => {
                // Short enough for the names' room; the tooltip says more.
                format!("Other · {}", crate::app::format_count(self.other_count()))
            }
        }
    }

    /// The full name of a lane, for its tooltip.
    pub fn lane_title(&self, lane: Lane) -> String {
        match lane {
            Lane::Other => {
                let n = self.other_count();
                let noun = if n == 1 { "value" } else { "values" };
                format!("Other: {} {noun} with no lane of their own", crate::app::format_count(n))
            }
            Lane::Empty => "Lines with no value in this column".to_string(),
            lane => self.lane_name(lane),
        }
    }

    /// The color of a file a lane stands for, when the lanes split by the
    /// Source or Log Name Column: the tint its rows have in the list.
    pub fn lane_tint(&self, lane: Lane, dark: bool) -> Option<gpui::Hsla> {
        let Lane::Value(_) = lane else { return None };
        let key = self.histogram.lanes_for.as_deref()?;
        let name = self.lane_name(lane);
        let space = self.active_space()?;
        // The first file of the lane's log: the files of one log each have
        // their own tint.
        let file = space.files.iter().find(|f| match key {
            SOURCE_KEY => f.display_name == name,
            LOG_NAME_KEY => crate::rotation_name::log_name(&f.display_name) == name,
            _ => false,
        })?;
        Some(crate::ui::canvas::source_tint(space.source_tint_index(&file.file_id)?, dark))
    }

    /// Whether a lane's value is ticked in the Filter of its column.
    pub fn lane_ticked(&self, lane: Lane) -> bool {
        let (Lane::Value(_), Some(key)) = (lane, self.histogram.lanes_for.as_deref()) else { return false };
        let folded = crate::unicode_text::fold(&self.lane_name(lane));
        self.filter_state.is_value_ticked(key, &folded)
    }

    /// A lane's name was clicked: its value is ticked in the Filter of the
    /// column, or unticked when it was, as its checkbox in the Filter
    /// sidebar does (in Hide Mode, ticked hides it). "Other" and "(empty)"
    /// do nothing.
    pub fn lane_name_clicked(&mut self, lane: Lane, cx: &mut Context<Self>) {
        let Lane::Value(_) = lane else { return };
        let Some(key) = self.histogram.lanes_for.clone() else { return };
        let value = self.lane_name(lane);
        if !value.is_empty() {
            self.filter_toggle_value(&key, &value, cx);
        }
    }

    /// The pointer is at time `at` over `lane`: say what is there, its
    /// lines counted once each time it moves to another column.
    pub fn lane_hover_at(&mut self, lane: Lane, at: f64, cx: &mut Context<Self>) {
        let Some(col) = self.histogram.shown.grid.and_then(|g| g.column_of(at)) else { return self.lane_hover_clear(cx) };
        if self.histogram.lane_hover.as_ref().is_some_and(|h| h.lane == lane && h.col == col) {
            return;
        }
        let (mut lines, mut level) = (0, None);
        {
            let rows = self.rows();
            self.each_lane_row(lane, col, |row| {
                lines += 1;
                let l = rows.level(row).unwrap_or(LogLevel::Unknown);
                if level.is_none_or(|top| severity(l) > severity(top)) {
                    level = Some(l);
                }
                true
            });
        }
        self.histogram.lane_hover = Some(LaneHover { lane, col, lines, level });
        cx.notify();
    }

    /// The pointer left the Swimlanes.
    pub fn lane_hover_clear(&mut self, cx: &mut Context<Self>) {
        if self.histogram.lane_hover.take().is_some() {
            cx.notify();
        }
    }

    /// What the pointer is over, as the Swimlane strip says it:
    /// "db.log · 10:30:00–10:31:00 · 12 lines · ERROR".
    pub fn lane_hover_text(&self) -> Option<String> {
        let hover = self.histogram.lane_hover.as_ref()?;
        let grid = self.histogram.shown.grid?;
        let step = self.histogram.axis.as_ref().map_or(th::Step(1.0), |a| a.step);
        let anchor = self.filter_index.read(&self.view.rows).anchor_day();
        let (from, to) = th::format_time_range(grid.start_of(hover.col), grid.start_of(hover.col + 1), anchor, step);
        let name = self.lane_name(hover.lane);
        if hover.lines == 0 {
            return Some(format!("{name} · {from}–{to} · no lines"));
        }
        let lines = if hover.lines == 1 { "1 line".to_string() } else { format!("{} lines", crate::app::format_count(hover.lines)) };
        let level = hover.level.filter(|l| *l != LogLevel::Unknown).map_or(String::new(), |l| format!(" · {}", l.as_str()));
        Some(format!("{name} · {from}–{to} · {lines}{level}"))
    }

    /// A lane's bar in column `col` was clicked: go to the first Parent
    /// Line of the lane in that column. The Filter stays as it is.
    pub fn lane_bar_clicked(&mut self, lane: Lane, col: usize, cx: &mut Context<Self>) {
        if let Some(row) = self.first_row_of_lane(lane, col) {
            self.histogram_jump_to_row(row, cx);
        }
    }

    /// The first Parent Line of `lane` in column `col` of the counted grid.
    pub fn first_row_of_lane(&self, lane: Lane, col: usize) -> Option<usize> {
        let mut first = None;
        self.each_lane_row(lane, col, |row| {
            first = Some(row);
            false
        });
        first
    }

    /// Call `f` with each Parent Line of `lane` in column `col` of the
    /// counted grid, in order, until it returns false. The view's index is
    /// read (its files locked) meanwhile.
    fn each_lane_row(&self, lane: Lane, col: usize, mut f: impl FnMut(usize) -> bool) {
        let buckets = &self.histogram.shown;
        let (Some(grid), Some(lanes), Some(key)) = (buckets.grid, buckets.lanes.as_ref(), self.histogram.lanes_for.as_deref())
        else {
            return;
        };
        let index = self.filter_index.read(&self.view.rows);
        let Some(ids) = index.ids(key) else { return };
        let (start, end) = (grid.start_of(col), grid.start_of(col + 1));
        let sorted = self.view_times_ascend();
        let from = if sorted {
            match th::first_row_at(&index, start, true) {
                Some(row) => row,
                None => return,
            }
        } else {
            0
        };
        let rows = self.rows();
        for row in from..index.len() {
            let t = index.ts(row);
            if sorted && !(t < end) {
                break;
            }
            let in_col = t >= start && t < end && !rows.continues(row);
            if in_col && ids.id_of_row(row).is_some_and(|id| lanes.holds(lane, id)) && !f(row) {
                break;
            }
        }
    }
}
