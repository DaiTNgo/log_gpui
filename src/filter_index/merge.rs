//! The Filter columns of a Log Space from its files' own (task 4b.4): what
//! `FilterIndex::build_merged` gives on the merged rows, without reading
//! the rows again. Each file's value ids are mapped to ids of the Log
//! Space, the values in the order the merged rows first show them; the
//! counts, spellings and Coverage add up; the Source column holds each
//! file's name, and the Log Name column its Log Name. No value id is kept per merged row: a row finds its id
//! through its file's column and the map of that file (lean-view-memory,
//! decision 3).

use super::{ColumnIndex, ColumnValue, FilterIndex, MAX_DISTINCT, MISSING, Source};
use crate::index::view_rows::{ViewRows, unpack};
use crate::model::FieldKey;
use crate::unicode_text;

/// A file of the Log Space: its own Filter columns, and its name in the
/// Source column.
pub struct MergeFile<'a> {
    pub index: &'a FilterIndex,
    pub source: Option<&'a str>,
}

/// How a row of a Log Space finds its value id in one of the space's
/// columns.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ColumnMap {
    /// Not read by value ids: the column overflows, or a file's does.
    #[default]
    None,
    /// The Source column: the id of each file's name.
    Source(Vec<u16>),
    /// For each file, the position of the column among the file's columns
    /// and the space's id of each of that column's ids; `None` for a file
    /// without the column.
    Ids(Vec<Option<(usize, Vec<u16>)>>),
}

impl FilterIndex {
    /// The Filter columns of the merged rows `rows` (packed (file, line)) of
    /// `files`, with no value id per row, and for each column the map a row
    /// finds its id through. `field_order` is the union of the files'
    /// fields in column order, `own_ts` a line's own timestamp and
    /// `continues` whether a line is a Continuation Line.
    pub fn merge_files(
        files: &[MergeFile],
        rows: &ViewRows,
        field_order: &[String],
        own_ts: impl Fn(usize, usize) -> Option<f64>,
        continues: impl Fn(usize, usize) -> bool,
    ) -> (Self, Vec<ColumnMap>) {
        let mut index = FilterIndex::default();
        let mut maps = Vec::new();
        if rows.is_empty() {
            return (index, maps);
        }
        index.rows = rows.len();
        let mut sources = vec![
            Source::SourceFile,
            Source::LogName,
            Source::Stream,
            Source::Level,
            Source::Module,
            Source::RequestId,
            Source::TraceId,
        ];
        for name in field_order {
            let key = FieldKey::new(name);
            index.field_cols.insert(key);
            sources.push(Source::Field(key));
        }
        let first = first_rows(files, rows, &sources);
        for (source, values) in sources.into_iter().zip(&first.values) {
            let (column, map) = match source {
                Source::SourceFile => source_column(files, rows, &first.files),
                Source::LogName => log_name_column(files, rows, &first.files),
                _ => merge_column(source, files, rows, values),
            };
            index.columns.push(column);
            maps.push(map);
        }

        let own: Vec<&FilterIndex> = files.iter().map(|f| f.index).collect();
        add_up_totals(&mut index, &own);
        index.first_ts = rows.iter().find_map(|r| {
            let (file, pos) = unpack(r);
            own_ts(file, pos)
        });
        index.parent_row = (0..rows.len())
            .rev()
            .find(|&row| rows.get(row).is_some_and(|(file, pos)| !continues(file, pos)))
            .unwrap_or(0);
        (index, maps)
    }
}

/// The counts of lines and of `key=value` keys of `files` added up into
/// `index`, and the first of the earliest and of the latest time, in file
/// order as the merged rows have them.
pub(super) fn add_up_totals(index: &mut FilterIndex, files: &[&FilterIndex]) {
    index.parent_lines = 0;
    index.timed_lines = 0;
    index.kv_counts.clear();
    index.min_entry = None;
    index.max_entry = None;
    for file in files {
        index.parent_lines += file.parent_lines;
        index.timed_lines += file.timed_lines;
        for (key, n) in &file.kv_counts {
            *index.kv_counts.entry(*key).or_default() += n;
        }
        if let Some((ts, text)) = &file.min_entry {
            if index.min_entry.as_ref().is_none_or(|(t, _)| ts < t) {
                index.min_entry = Some((*ts, text.clone()));
            }
        }
        if let Some((ts, text)) = &file.max_entry {
            if index.max_entry.as_ref().is_none_or(|(t, _)| ts > t) {
                index.max_entry = Some((*ts, text.clone()));
            }
        }
    }
}

/// Where things first show among the merged rows (`u32::MAX`: never).
struct FirstRows {
    /// Each file's first line.
    files: Vec<u32>,
    /// Per column, per file, each value of the file's column; empty for a
    /// column not read by value ids.
    values: Vec<Vec<Vec<u32>>>,
}

/// One pass over the merged rows, until every file and every value of the
/// files' columns `sources` (those that keep value ids) has shown.
fn first_rows(files: &[MergeFile], rows: &ViewRows, sources: &[Source]) -> FirstRows {
    let cols: Vec<Vec<Option<&ColumnIndex>>> = sources
        .iter()
        .map(|source| {
            let own: Vec<Option<&ColumnIndex>> = files.iter().map(|f| f.index.columns.iter().find(|c| &c.source == source)).collect();
            let by_ids = own.iter().flatten().all(|c| !c.overflow && c.has_row_ids());
            if by_ids { own } else { vec![None; files.len()] }
        })
        .collect();
    let mut firsts = FirstRows {
        files: vec![u32::MAX; files.len()],
        values: cols.iter().map(|own| own.iter().map(|c| vec![u32::MAX; c.map_or(0, |c| c.values.len())]).collect()).collect(),
    };
    let mut left = files.len() + cols.iter().flatten().flatten().map(|c| c.values.iter().filter(|v| v.count > 0).count()).sum::<usize>();
    for (row, packed) in rows.iter().enumerate() {
        if left == 0 {
            break;
        }
        let (f, line) = unpack(packed);
        let Some(first) = firsts.files.get_mut(f) else { continue };
        if *first == u32::MAX {
            *first = row as u32;
            left -= 1;
        }
        for (own, values) in cols.iter().zip(&mut firsts.values) {
            let Some(col) = own[f] else { continue };
            let Some(id) = col.row_ids.get(line).filter(|&id| id != MISSING) else { continue };
            if let Some(slot) = values[f].get_mut(id as usize).filter(|slot| **slot == u32::MAX) {
                *slot = row as u32;
                left = left.saturating_sub(1);
            }
        }
    }
    firsts
}

/// The id of `local` (a file's value) among `col`'s values, the value
/// added when new; `None` past [`MAX_DISTINCT`] values.
pub(super) fn value_id(col: &mut ColumnIndex, local: &ColumnValue) -> Option<u16> {
    if let Some(&m) = col.folded_ids.get(&local.folded) {
        return Some(m);
    }
    if col.values.len() >= MAX_DISTINCT {
        return None;
    }
    let m = col.values.len() as u16;
    col.values.push(ColumnValue { display: local.display.clone(), folded: local.folded.clone(), count: 0 });
    col.folded_ids.insert(local.folded.clone(), m);
    Some(m)
}

/// The Source column: each row holds its file's name.
fn source_column(files: &[MergeFile], rows: &ViewRows, first_of_file: &[u32]) -> (ColumnIndex, ColumnMap) {
    name_column(Source::SourceFile, files, rows, first_of_file, |name| name)
}

/// The Log Name column: each row holds its file's Log Name, so the files
/// of one log (`app.log`, `app.log.1`) share a value.
fn log_name_column(files: &[MergeFile], rows: &ViewRows, first_of_file: &[u32]) -> (ColumnIndex, ColumnMap) {
    name_column(Source::LogName, files, rows, first_of_file, crate::rotation_name::log_name)
}

/// A column whose value is a file's: `name_of` its name, for every line of
/// it. A row finds its id through its file (`ColumnMap::Source`).
fn name_column(
    source: Source,
    files: &[MergeFile],
    rows: &ViewRows,
    first_of_file: &[u32],
    name_of: impl Fn(&str) -> &str,
) -> (ColumnIndex, ColumnMap) {
    let mut col = ColumnIndex::new(source, 0);
    col.rows = rows.len();
    // Files in the order their first row shows.
    let mut order: Vec<(u32, usize)> =
        first_of_file.iter().enumerate().filter(|(_, r)| **r != u32::MAX).map(|(f, &r)| (r, f)).collect();
    order.sort();
    let mut id_of_file: Vec<u16> = vec![MISSING; files.len()];
    for (_, f) in order {
        let Some(name) = files[f].source.map(&name_of).filter(|s| !s.is_empty()) else { continue };
        let lines = files[f].index.parent_lines;
        col.covered += lines;
        let folded = unicode_text::fold(name);
        let id = match col.folded_ids.get(&folded) {
            Some(&id) => id,
            None => {
                let id = col.values.len() as u16;
                col.values.push(ColumnValue { display: name.to_string(), folded: folded.clone(), count: 0 });
                col.folded_ids.insert(folded, id);
                id
            }
        };
        col.values[id as usize].count += lines;
        col.spellings.entry(name.to_string()).or_insert((id, 0)).1 += lines;
        id_of_file[f] = id;
    }
    if col.values.len() > MAX_DISTINCT {
        col.mark_overflow();
        return (col, ColumnMap::None);
    }
    col.finalize_display();
    (col, ColumnMap::Source(id_of_file))
}

/// A column of the files' own, with their value ids mapped to the merged
/// column's.
fn merge_column(source: Source, files: &[MergeFile], rows: &ViewRows, first_rows: &[Vec<u32>]) -> (ColumnIndex, ColumnMap) {
    let mut col = ColumnIndex::new(source.clone(), 0);
    col.rows = rows.len();
    let at: Vec<Option<usize>> = files.iter().map(|f| f.index.columns.iter().position(|c| c.source == source)).collect();
    let own: Vec<Option<&ColumnIndex>> = files.iter().zip(&at).map(|(f, at)| at.map(|at| &f.index.columns[at])).collect();
    col.covered = own.iter().flatten().map(|c| c.covered).sum();
    if own.iter().flatten().any(|c| c.overflow || !c.has_row_ids()) {
        col.mark_overflow();
        return (col, ColumnMap::None);
    }
    // Each value of each file, by the merged row it first shows in.
    let mut firsts: Vec<(u32, usize, u16)> = Vec::new();
    for (f, rows) in first_rows.iter().enumerate() {
        firsts.extend(rows.iter().enumerate().filter(|(_, r)| **r != u32::MAX).map(|(id, &r)| (r, f, id as u16)));
    }
    firsts.sort();
    let mut remap: Vec<Vec<u16>> = own.iter().map(|c| vec![MISSING; c.map_or(0, |c| c.values.len())]).collect();
    for (_, f, id) in firsts {
        let local = &own[f].expect("a file with values has the column").values[id as usize];
        let Some(merged) = value_id(&mut col, local) else {
            col.mark_overflow();
            return (col, ColumnMap::None);
        };
        col.values[merged as usize].count += local.count;
        remap[f][id as usize] = merged;
    }
    for (f, c) in own.iter().enumerate() {
        let Some(c) = c else { continue };
        for (raw, (id, n)) in &c.spellings {
            let merged = remap[f][*id as usize];
            if merged != MISSING {
                col.spellings.entry(raw.clone()).or_insert((merged, 0)).1 += n;
            }
        }
    }
    col.finalize_display();
    let map = at.into_iter().zip(remap).map(|(at, remap)| at.map(|at| (at, remap))).collect();
    (col, ColumnMap::Ids(map))
}

impl FilterIndex {
    /// The Filter values of `files` while they are indexed (task 5.2): each
    /// column's values and counts in the lines read so far, the files'
    /// values added together in file order. It holds no value id per row,
    /// so it filters nothing: it is what the picker shows until the files
    /// are read.
    pub fn values_so_far(files: &[MergeFile]) -> Self {
        let mut index = FilterIndex::default();
        let mut sources: Vec<Source> = Vec::new();
        for file in files {
            index.rows += file.index.rows;
            index.parent_lines += file.index.parent_lines;
            index.timed_lines += file.index.timed_lines;
            for col in &file.index.columns {
                if !sources.contains(&col.source) {
                    sources.push(col.source.clone());
                }
            }
        }
        for source in sources {
            if let Source::Field(key) = source {
                index.field_cols.insert(key);
            }
            let column = match source {
                Source::SourceFile => names_so_far(Source::SourceFile, files, |name| name),
                Source::LogName => names_so_far(Source::LogName, files, crate::rotation_name::log_name),
                _ => column_so_far(source, files),
            };
            index.columns.push(column);
        }
        index
    }
}

/// The Source or Log Name column so far: each file's `name_of` its name, on
/// its lines read so far.
fn names_so_far(source: Source, files: &[MergeFile], name_of: impl Fn(&str) -> &str) -> ColumnIndex {
    let mut col = ColumnIndex::new(source, 0);
    for file in files {
        col.rows += file.index.rows;
        let Some(name) = file.source.map(&name_of).filter(|s| !s.is_empty()) else { continue };
        let lines = file.index.parent_lines;
        col.covered += lines;
        let folded = unicode_text::fold(name);
        let id = match col.folded_ids.get(&folded) {
            Some(&id) => id,
            None => {
                let id = col.values.len() as u16;
                col.values.push(ColumnValue { display: name.to_string(), folded: folded.clone(), count: 0 });
                col.folded_ids.insert(folded, id);
                id
            }
        };
        col.values[id as usize].count += lines;
        col.spellings.entry(name.to_string()).or_insert((id, 0)).1 += lines;
    }
    if col.values.len() > MAX_DISTINCT {
        col.mark_overflow();
    } else {
        col.finalize_display();
    }
    col
}

/// A column of the files' own so far: their values by folded form, counts
/// and spellings added up.
fn column_so_far(source: Source, files: &[MergeFile]) -> ColumnIndex {
    let mut col = ColumnIndex::new(source.clone(), 0);
    let mut overflow = false;
    for file in files {
        col.rows += file.index.rows;
        let Some(own) = file.index.columns.iter().find(|c| c.source == source) else { continue };
        col.covered += own.covered;
        if overflow || own.overflow {
            overflow = true;
            continue;
        }
        let mut remap = vec![MISSING; own.values.len()];
        for (id, value) in own.values.iter().enumerate() {
            let Some(merged) = value_id(&mut col, value) else {
                overflow = true;
                break;
            };
            col.values[merged as usize].count += value.count;
            remap[id] = merged;
        }
        for (raw, (id, n)) in &own.spellings {
            if let Some(&merged) = remap.get(*id as usize).filter(|&&m| m != MISSING) {
                col.spellings.entry(raw.clone()).or_insert((merged, 0)).1 += n;
            }
        }
    }
    if overflow {
        col.mark_overflow();
    } else {
        col.finalize_display();
    }
    col
}
