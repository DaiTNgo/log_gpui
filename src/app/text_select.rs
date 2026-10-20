//! Logic: selecting text with the mouse across cells and rows of the Parsed
//! View, scrolling by itself when the pointer leaves the canvas, and copying
//! the stored text of the selection.

use super::AppView;
use crate::text_selection::{TextPos, TextSelection};
use crate::ui::canvas::horizontal_scroll_x;
use crate::ui::cells::{Hit, TextCell, byte_for_hit, cell_text, hit_cell};
use gpui::{Context, Pixels, Point, ScrollStrategy, Window, point, px};
use std::time::Duration;

/// Distance the canvas scrolls sideways each step while dragging past an edge.
const AUTOSCROLL_STEP_X: f32 = 24.0;
/// Time between two steps of the automatic scroll.
const AUTOSCROLL_INTERVAL: Duration = Duration::from_millis(40);

/// What a right-click landed on, kept for the context menu.
#[derive(Debug, Clone, PartialEq)]
pub struct CellTarget {
    /// Position of the row among the rows shown.
    pub row: usize,
    /// Byte of the stored line under the pointer.
    pub byte: usize,
    /// The cell under the pointer, when it has text.
    pub cell: Option<TextCell>,
    /// Where the pointer was, for opening the color bar next to it.
    pub at: Point<Pixels>,
    /// The column and value a rule made from this cell would use. `None` on
    /// TIMESTAMP and on a `_`.
    pub column_value: Option<(String, String)>,
}

impl AppView {
    /// Where a pointer at window position `at` is in the rows: the row (kept
    /// inside the rows shown) and the byte of its stored line. `None` when
    /// there are no rows.
    pub fn text_pos_at(&self, at: Point<Pixels>, window: &Window, cx: &gpui::App) -> Option<(TextPos, Hit)> {
        let total = self.visible_indices.len();
        if total == 0 {
            return None;
        }
        let bounds = self.canvas_bounds.get();
        let scroll = self.scroll_handle.0.borrow().base_handle.offset();
        let row_h = self.density.row_height();
        let y = f32::from(at.y - bounds.origin.y) - f32::from(scroll.y);
        let row = ((y / row_h).floor().max(0.0) as usize).min(total - 1);
        let x = f32::from(at.x - bounds.origin.x);
        let hit = hit_cell(&self.column_layout, x, f32::from(horizontal_scroll_x(&self.scroll_handle)));
        let entry = self.all_entries.get(*self.visible_indices.get(row)?)?;
        let font_size = self.density.font_size();
        let byte = byte_for_hit(entry, hit, |text, x| {
            crate::ui::mono::index_for_x(text, x, font_size, window, cx)
        });
        Some((TextPos::new(row, byte), hit))
    }

    pub fn start_text_selection(&mut self, at: Point<Pixels>, window: &Window, cx: &mut Context<Self>) {
        self.color_bar = None;
        self.pane_selection = None;
        match self.text_pos_at(at, window, cx) {
            Some((pos, _)) => {
                self.text_selection = Some(TextSelection::new(pos, pos));
                self.selecting = true;
            }
            None => self.text_selection = None,
        }
        cx.notify();
    }

    pub fn drag_text_selection(&mut self, at: Point<Pixels>, window: &Window, cx: &mut Context<Self>) {
        if !self.selecting {
            return;
        }
        if let (Some((pos, _)), Some(sel)) = (self.text_pos_at(at, window, cx), self.text_selection.as_mut()) {
            sel.move_head(pos);
        }
        self.autoscroll_toward(at);
        self.ensure_autoscroll(window, cx);
        cx.notify();
    }

    /// The mouse button was released: a drag leaves a selection (with the
    /// color bar next to it), a plain click leaves none.
    pub fn finish_text_selection(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) {
        if !self.selecting {
            return;
        }
        self.selecting = false;
        self.autoscroll_dir = (0, 0);
        match self.text_selection {
            Some(sel) if !sel.is_empty() => self.open_mark_bar(at, cx),
            _ => self.text_selection = None,
        }
        cx.notify();
    }

    pub fn clear_text_selection(&mut self, cx: &mut Context<Self>) {
        if self.text_selection.is_some() || self.color_bar.is_some() {
            self.text_selection = None;
            self.selecting = false;
            self.color_bar = None;
            cx.notify();
        }
    }

    /// Which way the canvas should scroll by itself for a pointer at `at`:
    /// `(horizontal, vertical)` each -1, 0 or 1.
    fn autoscroll_toward(&mut self, at: Point<Pixels>) {
        let b = self.canvas_bounds.get();
        let dx = if at.x < b.origin.x + self.column_layout.sticky_partition_width() {
            -1
        } else if at.x > b.origin.x + b.size.width {
            1
        } else {
            0
        };
        let dy = if at.y < b.origin.y {
            -1
        } else if at.y > b.origin.y + b.size.height {
            1
        } else {
            0
        };
        self.autoscroll_dir = (dx, dy);
        self.autoscroll_pointer = at;
    }

    /// While a drag is past an edge, keep scrolling and extending the
    /// selection even when the pointer does not move.
    fn ensure_autoscroll(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self.autoscroll_running || self.autoscroll_dir == (0, 0) {
            return;
        }
        self.autoscroll_running = true;
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(AUTOSCROLL_INTERVAL).await;
                let keep_going = this
                    .update_in(cx, |this, window, cx| {
                        if !this.selecting || this.autoscroll_dir == (0, 0) {
                            this.autoscroll_running = false;
                            return false;
                        }
                        this.step_autoscroll(window, cx);
                        true
                    })
                    .unwrap_or(false);
                if !keep_going {
                    break;
                }
            }
        })
        .detach();
    }

    fn step_autoscroll(&mut self, window: &Window, cx: &mut Context<Self>) {
        let (dx, dy) = self.autoscroll_dir;
        let total = self.visible_indices.len();
        if dy != 0 && total > 0 {
            // Scroll a row at a time by moving to the row past the edge.
            let (pos, _) = self.text_pos_at(self.autoscroll_pointer, window, cx).unzip();
            if let Some(pos) = pos {
                let next = if dy < 0 { pos.row.saturating_sub(1) } else { (pos.row + 1).min(total - 1) };
                self.scroll_handle.scroll_to_item(next, ScrollStrategy::Nearest);
            }
        }
        if dx != 0 {
            let handle = self.scroll_handle.0.borrow().base_handle.clone();
            let offset = handle.offset();
            let max = handle.max_offset();
            let x = (f32::from(offset.x) - dx as f32 * AUTOSCROLL_STEP_X).clamp(-f32::from(max.x).abs(), 0.0);
            handle.set_offset(point(px(x), offset.y));
        }
        if let (Some((pos, _)), Some(sel)) = (self.text_pos_at(self.autoscroll_pointer, window, cx), self.text_selection.as_mut()) {
            sel.move_head(pos);
        }
        cx.notify();
    }

    /// The stored text of the selection, one line per row shown. A row a
    /// Filter hides is not among the rows shown, so it is not copied.
    pub fn selected_text(&self) -> Option<String> {
        let sel = self.text_selection.filter(|s| !s.is_empty())?;
        let rows = self.visible_indices.iter().enumerate().filter_map(|(pos, &ix)| {
            self.all_entries.get(ix).map(|e| (pos, e.raw_line.as_str()))
        });
        Some(sel.copy_text(rows))
    }

    /// ⌘C in the Parsed View.
    pub fn copy_text_selection(&mut self, cx: &mut Context<Self>) {
        if let Some(text) = self.selected_text() {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
        }
    }

    /// What a right-click at `at` landed on.
    pub fn cell_target_at(&self, at: Point<Pixels>, window: &Window, cx: &gpui::App) -> Option<CellTarget> {
        let (pos, hit) = self.text_pos_at(at, window, cx)?;
        let entry = self.all_entries.get(*self.visible_indices.get(pos.row)?)?;
        let mut cell = None;
        let mut column_value = None;
        match hit {
            Hit::Text { cell: c, .. } => {
                if let Some((text, _)) = cell_text(entry, c) {
                    cell = Some(c);
                    column_value = match c {
                        TextCell::Timestamp | TextCell::Raw => None,
                        TextCell::Level | TextCell::Module | TextCell::Field(_) => {
                            Some((c.column_key().to_string(), text.to_string()))
                        }
                        // MESSAGE takes the selected text as a Custom Value.
                        TextCell::Message => self
                            .selected_text()
                            .map(|t| t.trim().to_string())
                            .filter(|t| !t.is_empty() && !t.contains('\n'))
                            .map(|t| (c.column_key().to_string(), t)),
                    };
                }
            }
            Hit::Source => {
                column_value = entry
                    .source
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .map(|s| ("source".to_string(), s.to_string()));
            }
            _ => {}
        }
        Some(CellTarget { row: pos.row, byte: pos.byte, cell, at, column_value })
    }

    /// Remember what a right-click landed on for the menu that opens. A click
    /// outside the selection drops the selection first.
    pub fn on_cell_right_click(&mut self, at: Point<Pixels>, window: &Window, cx: &mut Context<Self>) {
        let target = self.cell_target_at(at, window, cx);
        if let (Some(t), Some(sel)) = (&target, self.text_selection) {
            let inside = sel.range_in_row(t.row, usize::MAX).is_some_and(|r| r.contains(&t.byte))
                || sel.covers_row(t.row) && sel.ordered().0.row != sel.ordered().1.row;
            if !inside {
                self.text_selection = None;
            }
        }
        self.cell_target = target;
        cx.notify();
    }
}
