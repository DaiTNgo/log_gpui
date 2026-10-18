//! Logic: Markers painted on text, in the Parsed View. A Marker belongs to a
//! line of a file, so it shows in the Raw View and in every Log Space or File
//! View holding that file.

use super::AppView;
use crate::marker::{TextMarker, marker_at};
use crate::palette::HighlightColor;
use crate::space_view::apply_markers_merged;
use crate::workspace::log_space::LogFile;
use crate::workspace::working_set::TabTarget;
use gpui::Context;
use std::ops::Range;

/// A piece of the selection on one line: the line's entry and the bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinePiece {
    pub entry_ix: usize,
    pub range: Range<usize>,
}

impl AppView {
    /// The file a row of the open view belongs to.
    pub(crate) fn file_id_of_entry(&self, entry_ix: usize) -> Option<String> {
        let entry = self.all_entries.get(entry_ix)?;
        match entry.file_id.as_deref() {
            Some(id) => Some(id.to_string()),
            None => match self.active_target()? {
                TabTarget::FileView { file_id, .. } => Some(file_id),
                _ => None,
            },
        }
    }

    /// Run `f` on the stored file `file_id` of the active project.
    pub(crate) fn with_file_mut<R>(&mut self, file_id: &str, f: impl FnOnce(&mut LogFile) -> R) -> Option<R> {
        let project = self.active_project_mut()?;
        for space in &mut project.log_spaces {
            if let Some(file) = space.file_mut(file_id) {
                return Some(f(file));
            }
        }
        None
    }

    /// Put the stored Markers on the rows of the open view again.
    pub fn reapply_markers(&mut self) {
        let Some(target) = self.active_target() else { return };
        let Some(space_id) = target.space_id() else { return };
        let Some(space) = self.active_project().and_then(|p| p.log_space(space_id)).cloned() else {
            return;
        };
        match target {
            TabTarget::LogSpace { .. } => apply_markers_merged(&space, &mut self.all_entries),
            TabTarget::FileView { file_id, .. } => {
                if let Some(file) = space.file(&file_id) {
                    file.apply_markers(&mut self.all_entries);
                }
            }
            TabTarget::InvestigationBoard { .. } => {}
        }
    }

    /// The pieces of the text selection, one per line that has selected text.
    pub fn selection_pieces(&self) -> Vec<LinePiece> {
        let Some(sel) = self.text_selection else { return Vec::new() };
        let (start, end) = sel.ordered();
        let mut pieces = Vec::new();
        for row in start.row..=end.row {
            let Some(&entry_ix) = self.visible_indices.get(row) else { continue };
            let Some(entry) = self.all_entries.get(entry_ix) else { continue };
            if let Some(range) = sel.range_in_row(row, entry.raw_line.len()) {
                pieces.push(LinePiece { entry_ix, range });
            }
        }
        pieces
    }

    /// Paint the selected text with `color`: one Marker on each line. The
    /// color becomes the newest of the Recent row.
    pub fn mark_selection(&mut self, color: HighlightColor, cx: &mut Context<Self>) {
        let pieces = self.selection_pieces();
        let mut painted = false;
        for piece in pieces {
            let Some(file_id) = self.file_id_of_entry(piece.entry_ix) else { continue };
            let Some(entry) = self.all_entries.get(piece.entry_ix) else { continue };
            let (line, text, generation) = (entry.index, entry.raw_line.clone(), entry.generation);
            let c = color.clone();
            painted |= self
                .with_file_mut(&file_id, |file| file.paint_marker_in(file.generations_ago(generation), line, &text, piece.range.clone(), c))
                .unwrap_or(false);
        }
        if painted {
            self.note_recent_color(color);
            self.reapply_markers();
            self.schedule_session_save(cx);
        }
        self.text_selection = None;
        self.color_bar = None;
        cx.notify();
    }

    /// The Marker under a byte of a row, if any.
    pub fn marker_under(&self, vis_pos: usize, byte: usize) -> Option<TextMarker> {
        let &entry_ix = self.visible_indices.get(vis_pos)?;
        let entry = self.all_entries.get(entry_ix)?;
        marker_at(&entry.markers, byte).cloned()
    }

    /// Remove the whole Marker under a byte of a row.
    pub fn remove_marker_under(&mut self, vis_pos: usize, byte: usize, cx: &mut Context<Self>) {
        let Some(&entry_ix) = self.visible_indices.get(vis_pos) else { return };
        let Some(file_id) = self.file_id_of_entry(entry_ix) else { return };
        let Some(entry) = self.all_entries.get(entry_ix) else { return };
        let (line, text, generation) = (entry.index, entry.raw_line.clone(), entry.generation);
        let removed = self
            .with_file_mut(&file_id, |file| file.remove_marker_in(file.generations_ago(generation), line, &text, byte))
            .unwrap_or(false);
        if removed {
            self.reapply_markers();
            self.schedule_session_save(cx);
        }
        cx.notify();
    }

    /// Give the Marker under a byte of a row another color.
    pub fn recolor_marker_under(
        &mut self,
        vis_pos: usize,
        byte: usize,
        color: HighlightColor,
        cx: &mut Context<Self>,
    ) {
        let Some(&entry_ix) = self.visible_indices.get(vis_pos) else { return };
        let Some(file_id) = self.file_id_of_entry(entry_ix) else { return };
        let Some(entry) = self.all_entries.get(entry_ix) else { return };
        let (line, text, generation) = (entry.index, entry.raw_line.clone(), entry.generation);
        let c = color.clone();
        let changed = self
            .with_file_mut(&file_id, |file| file.recolor_marker_in(file.generations_ago(generation), line, &text, byte, c))
            .unwrap_or(false);
        if changed {
            self.note_recent_color(color);
            self.reapply_markers();
            self.schedule_session_save(cx);
        }
        self.color_bar = None;
        cx.notify();
    }

    /// Remember a color as the newest of the Recent row.
    pub fn note_recent_color(&mut self, color: HighlightColor) {
        if let Some(project) = self.active_project_mut() {
            project.recent_colors.push(color);
        }
    }
}
