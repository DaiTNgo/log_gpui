//! The Filter columns and the rows' times of the open view, read from its files'
//! own index instead of a copy of it (lean-view-memory, decisions 1–3).
//!
//! - A File View reads its file's `FilterIndex`, which Indexing keeps up
//!   to date, live lines included.
//! - A Log Space keeps only its columns' values, counts and Coverage (a
//!   `FilterIndex` with no value id per row) and, per column, a small map
//!   from each file's value ids to the space's. A row finds its id through
//!   its file's column, and its time through its file's `LineMeta`.
//!
//! Each reading takes the files' locks once (`ViewIndex::read`), with
//! `read_recursive`: the rows read meanwhile lock the same files again.

use super::merge::{add_up_totals, value_id};
use super::{ColumnIndex, ColumnMap, FilterIndex, MISSING, Source};
use crate::index::indexing::{Indexed, IndexedData};
use crate::index::meta::{NO_TIME, seconds};
use crate::index::view_rows::ViewRows;
use parking_lot::RwLockReadGuard;
use std::sync::Arc;

/// Where the open view's Filter columns and times come from.
#[derive(Debug, Clone)]
pub enum ViewIndex {
    /// Built for the view: the values read so far while its files are
    /// indexed, or none.
    Owned(Arc<FilterIndex>),
    /// A File View: its file's own index.
    File(Arc<Indexed>),
    /// A Log Space: its files' columns, their ids mapped to the space's.
    Space(Arc<SpaceIndex>),
}

impl Default for ViewIndex {
    fn default() -> Self {
        Self::Owned(Arc::default())
    }
}

impl From<FilterIndex> for ViewIndex {
    fn from(index: FilterIndex) -> Self {
        Self::Owned(Arc::new(index))
    }
}

/// The Filter columns of a Log Space.
#[derive(Debug, Clone, Default)]
pub struct SpaceIndex {
    /// The space's columns: values, counts, spellings, Coverage, counts of
    /// lines; no value id per row and no time per row.
    pub meta: FilterIndex,
    /// The files, in the view's file order (`None`: not read).
    files: Vec<Option<Arc<Indexed>>>,
    /// For each column of `meta`, in order, how a row finds its id.
    maps: Vec<ColumnMap>,
    /// The files with no timestamp at all: their rows have no time.
    untimed: Vec<bool>,
}

impl SpaceIndex {
    pub fn new(meta: FilterIndex, files: Vec<Option<Arc<Indexed>>>, maps: Vec<ColumnMap>, untimed: Vec<bool>) -> Self {
        debug_assert_eq!(meta.columns.len(), maps.len());
        Self { meta, files, maps, untimed }
    }

    /// Whether these are the files it reads (the same index of each).
    pub fn reads(&self, files: &[Option<Arc<Indexed>>]) -> bool {
        self.files.len() == files.len()
            && self.files.iter().zip(files).all(|(a, b)| match (a, b) {
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            })
    }

    /// The memory it holds on the heap, in bytes, roughly.
    pub fn heap_bytes(&self) -> usize {
        let maps: usize = self
            .maps
            .iter()
            .map(|m| match m {
                ColumnMap::None => 0,
                ColumnMap::Source(ids) => ids.len() * 2,
                ColumnMap::Ids(files) => files.iter().flatten().map(|(_, ids)| ids.len() * 2 + 16).sum(),
            })
            .sum();
        self.meta.heap_bytes() + maps
    }
}

impl SpaceIndex {
    /// Take in the files' new lines: the rows from `from` on of `rows` are
    /// new (appended, or merged in again with the new ones). Values first
    /// shown by them join the columns' values in the order they show;
    /// counts, spellings, Coverage and the counts of lines are the files'
    /// added up again. No line is read: the files' Indexing read them.
    /// False when only a build from scratch gives the columns (a column
    /// came or overflowed, too many values): nothing is changed then.
    pub fn append(&mut self, rows: &ViewRows, from: usize, untimed: Vec<bool>) -> bool {
        let guards: Vec<Option<RwLockReadGuard<IndexedData>>> =
            self.files.iter().map(|f| f.as_ref().map(|f| f.data.read_recursive())).collect();
        let own: Vec<Option<&FilterIndex>> = guards.iter().map(|g| g.as_ref().map(|d| &d.filter)).collect();
        // A column a file has and the space does not: built again.
        let known = |source: &Source| self.meta.columns.iter().any(|c| &c.source == source);
        if own.iter().flatten().any(|f| f.columns.iter().any(|c| !known(&c.source))) {
            return false;
        }
        // Per column, per file, the ids new since the last time.
        let mut maps = self.maps.clone();
        let mut pending: Vec<Vec<Vec<bool>>> = vec![Vec::new(); maps.len()];
        for (at, map) in maps.iter_mut().enumerate() {
            let ColumnMap::Ids(files) = map else { continue };
            let source = &self.meta.columns[at].source;
            pending[at] = vec![Vec::new(); files.len()];
            for (f, file) in own.iter().enumerate() {
                let Some(file) = file else { continue };
                let Some(pos) = file.columns.iter().position(|c| &c.source == source) else { continue };
                let col = &file.columns[pos];
                if col.overflow || !col.has_row_ids() {
                    return false;
                }
                let entry = files[f].get_or_insert_with(|| (pos, Vec::new()));
                if entry.0 != pos {
                    return false;
                }
                let have = entry.1.len();
                entry.1.resize(col.values.len(), MISSING);
                pending[at][f] = (0..col.values.len()).map(|id| id >= have).collect();
            }
        }
        let mut meta = self.meta.clone();
        // New values, in the order the new rows show them; a value no new
        // row shows comes after them.
        let has_pending = pending.iter().any(|files| files.iter().any(|ids| ids.contains(&true)));
        if has_pending {
            let mut order: Vec<(usize, usize, u16)> = Vec::new();
            for row in from..rows.len() {
                let Some((f, line)) = rows.get(row) else { continue };
                for (at, map) in maps.iter().enumerate() {
                    let ColumnMap::Ids(files) = map else { continue };
                    let (Some(Some((pos, _))), Some(file)) = (files.get(f), own[f]) else { continue };
                    let Some(id) = file.columns[*pos].id_of_row(line).filter(|&id| id != MISSING) else { continue };
                    if let Some(p) = pending[at].get_mut(f).and_then(|ids| ids.get_mut(id as usize)).filter(|p| **p) {
                        *p = false;
                        order.push((at, f, id));
                    }
                }
            }
            for (at, files) in pending.iter().enumerate() {
                for (f, ids) in files.iter().enumerate() {
                    order.extend(ids.iter().enumerate().filter(|(_, p)| **p).map(|(id, _)| (at, f, id as u16)));
                }
            }
            for (at, f, id) in order {
                let ColumnMap::Ids(files) = &mut maps[at] else { continue };
                let Some((pos, remap)) = files[f].as_mut() else { continue };
                let local = &own[f].expect("a file with a map").columns[*pos].values[id as usize];
                let Some(merged) = value_id(&mut meta.columns[at], local) else { return false };
                remap[id as usize] = merged;
            }
        }
        // Counts, spellings and Coverage: the files' added up again.
        for (at, map) in maps.iter().enumerate() {
            let col = &mut meta.columns[at];
            col.rows = rows.len();
            let file_col = |f: usize| own[f].and_then(|o| o.columns.iter().find(|c| c.source == col.source));
            match map {
                ColumnMap::None => {
                    col.covered = (0..own.len()).filter_map(|f| file_col(f)).map(|c| c.covered).sum();
                }
                ColumnMap::Source(ids) => {
                    col.covered = 0;
                    col.spellings.clear();
                    for v in &mut col.values {
                        v.count = 0;
                    }
                    for (f, &id) in ids.iter().enumerate() {
                        let Some(file) = own[f] else { continue };
                        if id == MISSING {
                            continue;
                        }
                        let lines = file.parent_lines;
                        col.covered += lines;
                        col.values[id as usize].count += lines;
                        let name = col.values[id as usize].display.clone();
                        col.spellings.entry(name).or_insert((id, 0)).1 += lines;
                    }
                    col.finalize_display();
                }
                ColumnMap::Ids(files) => {
                    col.covered = 0;
                    col.spellings.clear();
                    for v in &mut col.values {
                        v.count = 0;
                    }
                    for (f, map) in files.iter().enumerate() {
                        let (Some(file), Some((pos, remap))) = (own[f], map) else { continue };
                        let local = &file.columns[*pos];
                        col.covered += local.covered;
                        for (id, v) in local.values.iter().enumerate() {
                            if let Some(&m) = remap.get(id).filter(|&&m| m != MISSING) {
                                col.values[m as usize].count += v.count;
                            }
                        }
                        for (raw, (id, n)) in &local.spellings {
                            if let Some(&m) = remap.get(*id as usize).filter(|&&m| m != MISSING) {
                                col.spellings.entry(raw.clone()).or_insert((m, 0)).1 += n;
                            }
                        }
                    }
                    col.finalize_display();
                }
            }
        }
        let files: Vec<&FilterIndex> = own.iter().flatten().copied().collect();
        add_up_totals(&mut meta, &files);
        let meta_of = |f: usize, line: usize| guards.get(f)?.as_ref()?.file.meta.get(line);
        if meta.first_ts.is_none() {
            meta.first_ts = (from..rows.len()).find_map(|row| {
                let (f, line) = rows.get(row)?;
                meta_of(f, line).filter(|m| m.flags & crate::index::meta::flags::OWN_TS != 0).map(|m| seconds(m.ts_us))
            });
        }
        meta.parent_row = (0..rows.len())
            .rev()
            .find(|&row| rows.get(row).and_then(|(f, line)| meta_of(f, line)).is_some_and(|m| m.parent_dist == 0))
            .unwrap_or(0);
        meta.rows = rows.len();
        drop(guards);
        self.meta = meta;
        self.maps = maps;
        self.untimed = untimed;
        true
    }
}

impl ViewIndex {
    /// Read the index of the view whose rows are `rows`: the files' locks
    /// are held until the reading is dropped.
    pub fn read<'a>(&'a self, rows: &'a ViewRows) -> ViewIndexRead<'a> {
        match self {
            Self::Owned(index) => ViewIndexRead::Owned(index),
            Self::File(indexed) => ViewIndexRead::File(indexed.data.read_recursive()),
            Self::Space(space) => ViewIndexRead::Space {
                space,
                files: space.files.iter().map(|f| f.as_ref().map(|f| f.data.read_recursive())).collect(),
                rows,
            },
        }
    }

    /// The memory the view holds for it beyond its files' index, in bytes.
    pub fn own_heap_bytes(&self) -> usize {
        match self {
            Self::Owned(index) => index.heap_bytes(),
            Self::File(_) => 0,
            Self::Space(space) => space.heap_bytes(),
        }
    }
}

/// The view's index while it is read. It reads like a `FilterIndex` (the
/// columns, their values, Coverage, counts, time span); the value id and
/// time of a row come from `ids` and `effective_timestamp`.
pub enum ViewIndexRead<'a> {
    Owned(&'a FilterIndex),
    File(RwLockReadGuard<'a, IndexedData>),
    Space { space: &'a SpaceIndex, files: Vec<Option<RwLockReadGuard<'a, IndexedData>>>, rows: &'a ViewRows },
}

impl std::ops::Deref for ViewIndexRead<'_> {
    type Target = FilterIndex;

    fn deref(&self) -> &FilterIndex {
        match self {
            Self::Owned(index) => index,
            Self::File(data) => &data.filter,
            Self::Space { space, .. } => &space.meta,
        }
    }
}

impl FilterIndex {
    /// This index read as a view's: its rows are its own.
    pub fn read(&self) -> ViewIndexRead<'_> {
        ViewIndexRead::Owned(self)
    }
}

impl<'a> ViewIndexRead<'a> {
    /// The value ids of column `key` row by row; `None` when the column
    /// does not keep them (it overflows, or is not read yet): a condition
    /// on it reads the rows' text.
    pub fn ids(&self, key: &str) -> Option<RowIds<'_>> {
        match self {
            Self::Owned(index) => index.column(key).filter(|c| c.has_row_ids()).map(RowIds::Column),
            Self::File(data) => data.filter.column(key).filter(|c| c.has_row_ids()).map(RowIds::Column),
            Self::Space { space, files, rows } => {
                let at = space.meta.columns.iter().position(|c| c.key == key)?;
                match &space.maps[at] {
                    ColumnMap::None => None,
                    ColumnMap::Source(ids) => Some(RowIds::Source { rows, ids }),
                    ColumnMap::Ids(maps) => {
                        let cols = maps
                            .iter()
                            .zip(files)
                            .map(|(map, data)| {
                                let (at, remap) = map.as_ref()?;
                                Some((&data.as_ref()?.filter.columns[*at], remap.as_slice()))
                            })
                            .collect();
                        Some(RowIds::Space { rows, files: cols })
                    }
                }
            }
        }
    }

    /// Timestamp of `row` as the Time Range and the Time Histogram see it:
    /// the row's own, else the one it takes from above.
    pub fn effective_timestamp(&self, row: usize) -> Option<f64> {
        let t = self.ts(row);
        (!t.is_nan()).then_some(t)
    }

    /// `effective_timestamp` with `NAN` for none.
    pub fn ts(&self, row: usize) -> f64 {
        match self {
            Self::Owned(index) => index.effective_ts.get(row).copied().unwrap_or(f64::NAN),
            Self::File(data) => data.effective_timestamp(row).unwrap_or(f64::NAN),
            Self::Space { space, files, rows } => {
                let Some((file, line)) = rows.get(row) else { return f64::NAN };
                if space.untimed.get(file).copied().unwrap_or(true) {
                    return f64::NAN;
                }
                match files.get(file).and_then(|f| f.as_ref()).and_then(|data| data.file.meta.get(line)) {
                    Some(m) if m.ts_us != NO_TIME => seconds(m.ts_us),
                    _ => f64::NAN,
                }
            }
        }
    }

    /// Rows of the view.
    pub fn len(&self) -> usize {
        match self {
            Self::Space { rows, .. } => rows.len(),
            _ => self.rows(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// The value id of each row in one column.
pub enum RowIds<'a> {
    /// The column's own.
    Column(&'a ColumnIndex),
    /// A Log Space's Source column: the id of the row's file.
    Source { rows: &'a ViewRows, ids: &'a [u16] },
    /// A Log Space's column: the row's file's column, through that file's
    /// map (`None`: the file has no such column).
    Space { rows: &'a ViewRows, files: Vec<Option<(&'a ColumnIndex, &'a [u16])>> },
}

impl RowIds<'_> {
    /// Value id of `row` (`MISSING`: no value), or `None` when ids are not
    /// kept for it.
    pub fn id_of_row(&self, row: usize) -> Option<u16> {
        match self {
            Self::Column(col) => col.id_of_row(row),
            Self::Source { rows, ids } => {
                let (file, _) = rows.get(row)?;
                Some(ids.get(file).copied().unwrap_or(MISSING))
            }
            Self::Space { rows, files } => {
                let (file, line) = rows.get(row)?;
                let Some(Some((col, remap))) = files.get(file) else { return Some(MISSING) };
                Some(match col.id_of_row(line) {
                    Some(id) if id != MISSING => remap.get(id as usize).copied().unwrap_or(MISSING),
                    _ => MISSING,
                })
            }
        }
    }
}

impl<'a> RowIds<'a> {
    /// `table` (whether each of the column's values meets a condition) read
    /// row by row: for a Log Space, folded into a table per file, so a row
    /// takes one lookup in its file's ids.
    pub fn with_table(self, table: Vec<bool>) -> RowTable<'a> {
        let pick = |remap: &[u16]| remap.iter().map(|&m| m != MISSING && table.get(m as usize).copied().unwrap_or(false)).collect();
        match self {
            Self::Column(col) => RowTable::Column { col, table },
            Self::Source { rows, ids } => RowTable::Source { rows, files: pick(ids) },
            Self::Space { rows, files } => {
                RowTable::Space { rows, files: files.into_iter().map(|f| f.map(|(col, remap)| (col, pick(remap)))).collect() }
            }
        }
    }
}

/// Whether each row's value in a column meets a condition, by value ids.
pub enum RowTable<'a> {
    Column { col: &'a ColumnIndex, table: Vec<bool> },
    /// Whether each file's name meets it.
    Source { rows: &'a ViewRows, files: Vec<bool> },
    /// For each file, its column and whether each of its values meets it.
    Space { rows: &'a ViewRows, files: Vec<Option<(&'a ColumnIndex, Vec<bool>)>> },
}

impl RowTable<'_> {
    /// Whether `row` meets it (false with no value), or `None` when ids are
    /// not kept for it.
    pub fn get(&self, row: usize) -> Option<bool> {
        let of = |col: &ColumnIndex, table: &[bool], line: usize| {
            col.id_of_row(line).map(|id| id != MISSING && table.get(id as usize).copied().unwrap_or(false))
        };
        match self {
            Self::Column { col, table } => of(col, table, row),
            Self::Source { rows, files } => {
                let (file, _) = rows.get(row)?;
                Some(files.get(file).copied().unwrap_or(false))
            }
            Self::Space { rows, files } => {
                let (file, line) = rows.get(row)?;
                let Some(Some((col, table))) = files.get(file) else { return Some(false) };
                Some(of(col, table, line).unwrap_or(false))
            }
        }
    }
}
