//! Logic: what the views need to paint Text Highlights and Value Colors,
//! worked out once per frame. Which Field Columns get a Value Color is told
//! from a sample of the rows already read, taken a little more each frame
//! until it is full, so a frame never reads a file.

use super::AppView;
use crate::text_highlight::HighlightMatcher;
use crate::value_color::{FieldSampler, ValueColors, hues_to_avoid};
use gpui::Context;
use std::sync::Arc;

/// Rows looked at, at most, to tell the kind of each Field Column.
const SAMPLE_ROWS: usize = 4096;

/// The sample of the open view's Field Columns, and what it was taken for.
#[derive(Debug, Clone, Default)]
pub struct ValueSample {
    /// The view (tab, rows epoch and Field Columns) the sample is of.
    key: String,
    /// Rows looked at so far, from the first.
    rows: usize,
    sampler: FieldSampler,
}

impl AppView {
    /// Work out the paint state of Text Highlights and Value Colors for this
    /// frame. Called while rendering, before the views draw.
    pub fn refresh_text_paint(&mut self, cx: &mut Context<Self>) {
        let tone = self.tone(cx);
        self.th_matcher = Arc::new(HighlightMatcher::new(self.text_highlights()));
        self.value_colors = if self.workspace_state.show_value_colors {
            let fields = self.sample_value_fields();
            let avoid = hues_to_avoid(self.th_matcher.colors(), tone);
            Some(Arc::new(ValueColors { fields, avoid, tone }))
        } else {
            None
        };
    }

    /// The Field Columns that get a Value Color, sampling a few more rows of
    /// the view when the sample is not full yet.
    fn sample_value_fields(&mut self) -> std::collections::HashSet<crate::model::FieldKey> {
        let fields = self.column_layout.fields.clone();
        let key = format!("{:?}|{}|{:?}", self.active_target(), self.rows_epoch, fields);
        if self.value_sample.key != key {
            self.value_sample = ValueSample { key, ..Default::default() };
        }
        let sample = &mut self.value_sample;
        if !fields.is_empty() && !sample.sampler.is_full(&fields) && sample.rows < SAMPLE_ROWS {
            let rows = self.view.bind(&self.parsed_files);
            let end = rows.len().min(SAMPLE_ROWS);
            while sample.rows < end {
                // A row not read yet stops the sample until a later frame.
                let Some(entry) = rows.cached(sample.rows) else { break };
                sample.sampler.add(&entry, &fields);
                sample.rows += 1;
            }
        }
        sample.sampler.colored()
    }
}
