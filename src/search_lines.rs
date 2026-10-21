//! The texts of every row, kept for the Search scan that reads them on a
//! worker thread. Copying a million texts on the UI thread for every scan
//! took tens of milliseconds; here the old rows are copied once, and a scan
//! after Live lines arrived only copies those.
//!
//! The texts sit in blocks. A full block is shared and never changes, so a
//! scan under way can go on reading the snapshot it was given while the next
//! one grows; only the last, partly filled block is copied when rows join it.

use crate::model::LogEntry;
use gpui::SharedString;
use std::sync::Arc;

const BLOCK_BITS: usize = 16;
const BLOCK: usize = 1 << BLOCK_BITS;

#[derive(Clone, Default)]
pub struct SearchLines {
    blocks: Vec<Arc<Vec<SharedString>>>,
    len: usize,
    /// The `rows_epoch` the texts were taken at: rows may have left or moved
    /// since another one.
    epoch: u64,
}

impl SearchLines {
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn get(&self, row: usize) -> &str {
        self.blocks[row >> BLOCK_BITS][row & (BLOCK - 1)].as_str()
    }

    /// Bring the snapshot up to `rows`, which only grew at the end unless
    /// `epoch` differs from the one of the last sync. Returns how many texts
    /// were copied.
    pub fn sync(&mut self, rows: &[LogEntry], epoch: u64) -> usize {
        if epoch != self.epoch || rows.len() < self.len {
            *self = Self { epoch, ..Self::default() };
        }
        let from = self.len;
        if from == rows.len() {
            return 0;
        }
        let mut next = from;
        while next < rows.len() {
            let room = BLOCK - next % BLOCK;
            let end = (next + room).min(rows.len());
            let reopened = next % BLOCK != 0;
            let mut texts = if reopened { (**self.blocks.last().expect("a partly filled block")).clone() } else { Vec::new() };
            texts.extend(rows[next..end].iter().map(|e| e.raw_line.clone()));
            if reopened {
                *self.blocks.last_mut().expect("a partly filled block") = Arc::new(texts);
            } else {
                self.blocks.push(Arc::new(texts));
            }
            next = end;
        }
        self.len = rows.len();
        debug_assert!(
            rows.iter().enumerate().all(|(i, e)| self.get(i) == e.raw_line.as_str()),
            "the Search snapshot does not match the rows: a place changing rows did not bump rows_epoch"
        );
        rows.len() - from
    }
}
