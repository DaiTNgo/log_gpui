//! Logic: the Swimlanes under the Time Histogram (openspec change
//! `add-swimlane`): which column they split by (Lane By, kept with the
//! view's Filter), what the "Lane by" menu offers, and what clicking a lane
//! does. The lanes are counted with the histogram (`time_histogram`).

use super::AppView;
use crate::time_histogram::{self as th, Lane, LaneBy, LaneIds};
use crate::workspace::working_set::TabTarget;
use gpui::Context;

/// Field Columns the menu suggests, after MODULE, first found first.
const SUGGESTED_FIELDS: [&str; 6] = ["service", "module", "component", "thread", "host", "pod"];

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
    /// chosen column the view no longer has falls back to it (decision 5).
    pub fn lane_column(&self) -> Option<String> {
        let chosen = match self.lane_by() {
            LaneBy::None => return None,
            LaneBy::Column(key) if self.filter_index.column(&key).is_some() => Some(key),
            _ => None,
        };
        let key = chosen.or_else(|| {
            let log_name = self.filter_index.column("log_name")?;
            (self.active_is_log_space() && log_name.values.len() >= 2).then(|| "log_name".to_string())
        })?;
        self.filter_index.column(&key).filter(|c| c.has_row_ids()).map(|_| key)
    }

    /// The column the menu puts first as a suggestion while there are no
    /// lanes: MODULE, else the first Field Column named like a service.
    pub fn lane_suggestion(&self) -> Option<String> {
        if self.lane_column().is_some() {
            return None;
        }
        let in_use = |key: &str| self.filter_index.columns_in_use().any(|c| c.key == key);
        if in_use("module") {
            return Some("module".to_string());
        }
        SUGGESTED_FIELDS.iter().find_map(|name| {
            self.filter_index.field_columns().find(|c| c.key.eq_ignore_ascii_case(name)).map(|c| c.key.clone())
        })
    }

    /// The columns of the "Lane by" menu: every Filterable Column of the
    /// view, and those that cannot be chosen with the reason.
    pub fn lane_choices(&self) -> Vec<LaneChoice> {
        let suggested = self.lane_suggestion();
        let mut choices: Vec<LaneChoice> = self
            .filter_index
            .columns_in_use()
            .map(|c| LaneChoice {
                key: c.key.clone(),
                label: c.label.clone(),
                unavailable: if c.overflow {
                    Some(format!("Too many values (> {})", crate::filter_index::MAX_DISTINCT))
                } else if !c.has_row_ids() {
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

    /// The value ids of each row in the column the lanes split by.
    pub(crate) fn lane_ids(&self, key: &str) -> Option<LaneIds<'_>> {
        let col = self.filter_index.column(key).filter(|c| c.has_row_ids())?;
        Some(LaneIds { ids: col.row_ids(), values: col.values.len() })
    }

    /// The name a lane shows.
    pub fn lane_name(&self, lane: Lane) -> String {
        match lane {
            Lane::Value(v) => self
                .histogram
                .lanes_for
                .as_deref()
                .and_then(|key| self.filter_index.column(key))
                .and_then(|c| c.values.get(usize::from(v)))
                .map_or_else(String::new, |v| v.display.clone()),
            Lane::Empty => "(empty)".to_string(),
            Lane::Other => {
                let n = self.histogram.shown.lanes.as_ref().map_or(0, |l| l.set.other_count);
                let noun = if n == 1 { "value" } else { "values" };
                format!("Other ({} {noun})", crate::app::format_count(n))
            }
        }
    }

    /// A lane's name was clicked: its value is ticked in the Filter of the
    /// column, as ticking it in the Filter sidebar (in Hide Mode it hides
    /// it). "Other" and "(empty)" do nothing.
    pub fn lane_name_clicked(&mut self, lane: Lane, cx: &mut Context<Self>) {
        let Lane::Value(_) = lane else { return };
        let Some(key) = self.histogram.lanes_for.clone() else { return };
        let value = self.lane_name(lane);
        let folded = crate::unicode_text::fold(&value);
        let ticked = self.filter_state.column(&key).is_some_and(|c| c.values.contains(&folded));
        if !value.is_empty() && !ticked {
            self.filter_toggle_value(&key, &value, cx);
        }
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
        let buckets = &self.histogram.shown;
        let (grid, lanes) = (buckets.grid?, buckets.lanes.as_ref()?);
        let column = self.filter_index.column(self.histogram.lanes_for.as_deref()?)?;
        let (start, end) = (grid.start_of(col), grid.start_of(col + 1));
        let ts = self.filter_index.effective_ts();
        let sorted = self.view_times_ascend();
        let from = if sorted { th::first_row_at(ts, start, true)? } else { 0 };
        let rows = self.rows();
        (from..ts.len())
            .take_while(|&r| !sorted || ts[r] < end)
            .filter(|&r| ts[r] >= start && ts[r] < end && !rows.continues(r))
            .find(|&r| column.id_of_row(r).is_some_and(|id| lanes.holds(lane, id)))
    }
}
