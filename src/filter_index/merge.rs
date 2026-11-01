//! The Filter columns of a Log Space from its files' own (task 4b.4): what
//! `FilterIndex::build_merged` gives on the merged rows, without reading
//! the rows again. Each file's value ids are mapped to ids of the Log
//! Space, the values in the order the merged rows first show them; the
//! counts, spellings and Coverage add up; the Source column holds each
//! file's name.

use super::{ColumnIndex, ColumnValue, FilterIndex, MAX_DISTINCT, MISSING, Source};
use crate::index::view_rows::unpack;
use crate::model::FieldKey;
use crate::unicode_text;

/// A file of the Log Space: its own Filter columns, and its name in the
/// Source column.
pub struct MergeFile<'a> {
    pub index: &'a FilterIndex,
    pub source: Option<&'a str>,
}

impl FilterIndex {
    /// The Filter columns of the merged rows `rows` (packed (file, line)) of
    /// `files`. `field_order` is the union of the files' fields in column
    /// order, `effective_ts` the timestamp each merged row sorts by,
    /// `own_ts` a line's own timestamp and `continues` whether a line is a
    /// Continuation Line.
    pub fn merge_files(
        files: &[MergeFile],
        rows: &[u64],
        field_order: &[String],
        effective_ts: Vec<f64>,
        own_ts: impl Fn(usize, usize) -> Option<f64>,
        continues: impl Fn(usize, usize) -> bool,
    ) -> Self {
        let mut index = FilterIndex::default();
        if rows.is_empty() {
            return index;
        }
        index.rows = rows.len();
        // Where each line of each file is among the merged rows.
        let mut row_of: Vec<Vec<u32>> = files.iter().map(|f| vec![u32::MAX; f.index.rows()]).collect();
        for (row, &packed) in rows.iter().enumerate() {
            let (file, pos) = unpack(packed);
            if let Some(slot) = row_of.get_mut(file).and_then(|r| r.get_mut(pos)) {
                *slot = row as u32;
            }
        }
        let mut sources = vec![
            Source::SourceFile,
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
        for source in sources {
            let column = if source == Source::SourceFile {
                source_column(files, rows, &row_of)
            } else {
                merge_column(source, files, rows, &row_of)
            };
            index.columns.push(column);
        }

        for file in files {
            index.parent_lines += file.index.parent_lines;
            index.timed_lines += file.index.timed_lines;
            for (key, n) in &file.index.kv_counts {
                *index.kv_counts.entry(*key).or_default() += n;
            }
            // The first of the earliest and of the latest, in file order as
            // the merged rows have them.
            if let Some((ts, text)) = &file.index.min_entry {
                if index.min_entry.as_ref().is_none_or(|(t, _)| ts < t) {
                    index.min_entry = Some((*ts, text.clone()));
                }
            }
            if let Some((ts, text)) = &file.index.max_entry {
                if index.max_entry.as_ref().is_none_or(|(t, _)| ts > t) {
                    index.max_entry = Some((*ts, text.clone()));
                }
            }
        }
        index.first_ts = rows.iter().find_map(|&r| {
            let (file, pos) = unpack(r);
            own_ts(file, pos)
        });
        index.parent_row = rows
            .iter()
            .rposition(|&r| {
                let (file, pos) = unpack(r);
                !continues(file, pos)
            })
            .unwrap_or(0);
        index.effective_ts = std::sync::Arc::new(effective_ts);
        index
    }
}

/// The Source column: each row holds its file's name.
fn source_column(files: &[MergeFile], rows: &[u64], row_of: &[Vec<u32>]) -> ColumnIndex {
    let mut col = ColumnIndex::new(Source::SourceFile, 0);
    col.rows = rows.len();
    // Files in the order their first row shows.
    let mut order: Vec<(u32, usize)> = row_of
        .iter()
        .enumerate()
        .filter_map(|(f, rows)| rows.iter().copied().filter(|&r| r != u32::MAX).min().map(|r| (r, f)))
        .collect();
    order.sort();
    let mut id_of_file: Vec<u16> = vec![MISSING; files.len()];
    for (_, f) in order {
        let Some(name) = files[f].source.filter(|s| !s.is_empty()) else { continue };
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
        return col;
    }
    col.row_ids = rows.iter().map(|&r| id_of_file[unpack(r).0]).collect();
    col.finalize_display();
    col
}

/// A column of the files' own, with their value ids mapped to the merged
/// column's.
fn merge_column(source: Source, files: &[MergeFile], rows: &[u64], row_of: &[Vec<u32>]) -> ColumnIndex {
    let mut col = ColumnIndex::new(source.clone(), 0);
    col.rows = rows.len();
    let own: Vec<Option<&ColumnIndex>> =
        files.iter().map(|f| f.index.columns.iter().find(|c| c.source == source)).collect();
    col.covered = own.iter().flatten().map(|c| c.covered).sum();
    if own.iter().flatten().any(|c| c.overflow || !c.has_row_ids()) {
        col.mark_overflow();
        return col;
    }
    // Each value of each file, by the merged row it first shows in.
    let mut firsts: Vec<(u32, usize, u16)> = Vec::new();
    for (f, c) in own.iter().enumerate() {
        let Some(c) = c else { continue };
        let mut seen = vec![false; c.values.len()];
        for (pos, &id) in c.row_ids.iter().enumerate() {
            if id != MISSING && !seen[id as usize] {
                seen[id as usize] = true;
                firsts.push((row_of[f].get(pos).copied().unwrap_or(u32::MAX), f, id));
            }
        }
    }
    firsts.sort();
    let mut remap: Vec<Vec<u16>> = own.iter().map(|c| vec![MISSING; c.map_or(0, |c| c.values.len())]).collect();
    for (_, f, id) in firsts {
        let local = &own[f].expect("a file with values has the column").values[id as usize];
        let merged = match col.folded_ids.get(&local.folded) {
            Some(&m) => m,
            None => {
                if col.values.len() >= MAX_DISTINCT {
                    col.mark_overflow();
                    return col;
                }
                let m = col.values.len() as u16;
                col.values.push(ColumnValue { display: local.display.clone(), folded: local.folded.clone(), count: 0 });
                col.folded_ids.insert(local.folded.clone(), m);
                m
            }
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
    col.row_ids = rows
        .iter()
        .map(|&r| {
            let (f, pos) = unpack(r);
            match own.get(f).copied().flatten() {
                Some(c) => match c.row_ids.get(pos).copied() {
                    Some(id) if id != MISSING => remap[f][id as usize],
                    _ => MISSING,
                },
                None => MISSING,
            }
        })
        .collect();
    col.finalize_display();
    col
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
            let column = if source == Source::SourceFile {
                names_so_far(files)
            } else {
                column_so_far(source, files)
            };
            index.columns.push(column);
        }
        index
    }
}

/// The Source column so far: each file's name, on its lines read so far.
fn names_so_far(files: &[MergeFile]) -> ColumnIndex {
    let mut col = ColumnIndex::new(Source::SourceFile, 0);
    for file in files {
        col.rows += file.index.rows;
        let Some(name) = file.source.filter(|s| !s.is_empty()) else { continue };
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
            let merged = match col.folded_ids.get(&value.folded) {
                Some(&m) => m,
                None => {
                    if col.values.len() >= MAX_DISTINCT {
                        overflow = true;
                        break;
                    }
                    let m = col.values.len() as u16;
                    col.values.push(ColumnValue { display: value.display.clone(), folded: value.folded.clone(), count: 0 });
                    col.folded_ids.insert(value.folded.clone(), m);
                    m
                }
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
