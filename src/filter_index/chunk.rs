//! Building a `FilterIndex` from blocks of lines read in parallel (Indexing,
//! decision 7 of `line-offset-index`). A worker reads its block into a
//! `ChunkColumns`: the values of each column as ids of its own, with no
//! `LogEntry` kept. The block is then added in file order with
//! `FilterIndex::append_chunk`, which gives what `append` gives on the
//! entries of those lines.
//!
//! A worker does not know whether the lines at the top of its block that
//! have no time continue a line of an earlier block (Continuation Lines) or
//! start the file (Parent Lines): it reads them both ways, and the block is
//! settled when it is added.

use super::{ColumnIndex, FilterIndex, MISSING, Source, has_timestamp};
use crate::column::FieldMaxChars;
use crate::model::{FieldKey, LogEntry, Stream};
use crate::unicode_text;
use std::collections::{HashMap, HashSet};

/// What indexing needs to know of one line: its time, and the value of
/// every column it has. Borrowed from the line's text or from its entry.
#[derive(Debug, Clone, PartialEq)]
pub struct LineFacts<'a> {
    /// The line has a time of its own (it is not a Continuation Line).
    pub has_time: bool,
    /// That time, when it is a usable timestamp.
    pub timestamp: Option<f64>,
    pub level: crate::model::LogLevel,
    pub time_str: &'a str,
    pub level_text: &'a str,
    pub tag: &'a str,
    pub request_id: &'a str,
    pub trace_id: &'a str,
    pub source: Option<&'a str>,
    pub stream: Option<Stream>,
    /// The fields read with the line (a JSON key, a Log Format's), in order.
    pub fields: Vec<(FieldKey, &'a str)>,
    /// The Detected Fields read from the message, when the line is read as
    /// a Parent Line; a Continuation Line has none.
    pub extra: Vec<(FieldKey, &'a str)>,
    /// The `key=value` keys of the message.
    pub kv_keys: Vec<FieldKey>,
}

impl<'a> LineFacts<'a> {
    /// The facts of a parsed line. Its Detected Fields are those of its
    /// `extra`, read as if it were a Parent Line.
    pub fn of_entry(entry: &'a LogEntry) -> Self {
        let text = |r: std::ops::Range<usize>| entry.raw_line.get(r);
        Self {
            has_time: entry.has_time,
            timestamp: has_timestamp(entry).then_some(entry.timestamp),
            level: entry.level,
            time_str: entry.time_str(),
            level_text: entry.level_text(),
            tag: entry.tag(),
            request_id: entry.request_id.as_deref().unwrap_or(""),
            trace_id: entry.trace_id.as_deref().unwrap_or(""),
            source: entry.source.as_deref(),
            stream: entry.stream,
            fields: entry.spans.fields().filter_map(|(k, r)| Some((k, text(r)?))).collect(),
            extra: entry
                .extra
                .fields()
                .filter(|(k, _)| entry.spans.get(k.as_str()).is_none())
                .filter_map(|(k, r)| Some((k, text(r)?)))
                .collect(),
            kv_keys: crate::extract::kv_keys(entry),
        }
    }

    /// The text of field `key`: the line's own, else a Detected Field.
    fn field(&self, key: FieldKey, as_parent: bool) -> Option<&'a str> {
        let extra = if as_parent { &self.extra[..] } else { &[] };
        self.fields.iter().chain(extra).find(|(k, _)| *k == key).map(|(_, v)| *v)
    }

    fn value(&self, source: &Source, as_parent: bool) -> Option<&'a str> {
        let v = match source {
            Source::SourceFile => self.source.unwrap_or(""),
            Source::Stream => self.stream.map_or("", |s| s.as_str()),
            Source::Level => self.level_text,
            Source::Module => self.tag,
            Source::RequestId => self.request_id,
            Source::TraceId => self.trace_id,
            Source::Field(k) => self.field(*k, as_parent).unwrap_or(""),
        };
        if v.is_empty() { None } else { Some(v) }
    }
}

/// Id of a line that has a value in a column whose values are no longer
/// kept (the column overflowed).
const PRESENT: u16 = MISSING - 1;

/// How a worker reads a line of its block.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Role {
    /// It has a time: a Parent Line.
    Parent,
    /// No time, below a line of the block that has one.
    Continuation,
    /// No time, above every line of the block that has one: settled when
    /// the block is added.
    Unsettled,
}

/// One column's values in a block.
#[derive(Debug)]
struct LocalColumn {
    source: Source,
    /// Where in the block the column first shows, as (line, place among
    /// the line's fields), when that holds however the block is settled.
    first: Option<(usize, usize)>,
    /// The same, when the Unsettled lines turn out to be Parent Lines.
    first_if_parents: Option<(usize, usize)>,
    /// Per line: an id into `spellings`, `MISSING` or `PRESENT`.
    ids: Vec<u16>,
    /// Each spelling met, with its folded form.
    spellings: Vec<(String, String)>,
    lookup: HashMap<String, u16>,
    /// The column overflowed already: only whether a line has a value.
    presence_only: bool,
}

impl LocalColumn {
    fn new(source: Source, rows_before: usize, presence_only: bool) -> Self {
        Self {
            source,
            first: None,
            first_if_parents: None,
            ids: vec![MISSING; rows_before],
            spellings: Vec::new(),
            lookup: HashMap::new(),
            presence_only,
        }
    }

    fn record(&mut self, value: Option<&str>) {
        let id = match value {
            None => MISSING,
            Some(_) if self.presence_only => PRESENT,
            Some(v) => match self.lookup.get(v) {
                Some(&id) => id,
                None if self.spellings.len() >= usize::from(PRESENT) => {
                    // More spellings than ids: the column overflows anyway.
                    self.presence_only = true;
                    PRESENT
                }
                None => {
                    let id = self.spellings.len() as u16;
                    self.spellings.push((v.to_string(), unicode_text::fold(v)));
                    self.lookup.insert(v.to_string(), id);
                    id
                }
            },
        };
        self.ids.push(id);
    }
}

/// The columns of one block of lines, read by a worker.
#[derive(Debug, Default)]
pub struct ChunkColumns {
    rows: usize,
    roles: Vec<Role>,
    seen_parent: bool,
    columns: Vec<LocalColumn>,
    field_at: HashMap<FieldKey, usize>,
    /// Columns that overflowed before the block was read.
    overflowed: Overflowed,
    kv: HashMap<FieldKey, usize>,
    kv_if_parents: HashMap<FieldKey, usize>,
    /// Lengths of every line, and of the fields of its Parent Lines.
    pub max_chars: FieldMaxChars,
    /// Lengths of the fields of the Unsettled lines.
    max_if_parents: FieldMaxChars,
    timestamps: Vec<f64>,
    min_entry: Option<(f64, String)>,
    max_entry: Option<(f64, String)>,
}

const FIXED: [Source; 6] = [
    Source::SourceFile,
    Source::Stream,
    Source::Level,
    Source::Module,
    Source::RequestId,
    Source::TraceId,
];

fn add_lengths(max: &mut FieldMaxChars, fields: &[(FieldKey, &str)]) {
    for (key, text) in fields {
        let n = text.chars().count();
        let m = max.fields.entry(*key).or_default();
        *m = (*m).max(n);
    }
}

impl ChunkColumns {
    /// A block read knowing which columns overflowed already (their values
    /// are not kept).
    pub fn new(overflowed: Overflowed) -> Self {
        let columns = FIXED
            .iter()
            .map(|s| LocalColumn::new(s.clone(), 0, overflowed.0.contains(s)))
            .collect();
        Self { columns, overflowed, ..Self::default() }
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    fn field_column(&mut self, key: FieldKey) -> &mut LocalColumn {
        let at = match self.field_at.get(&key) {
            Some(&at) => at,
            None => {
                let source = Source::Field(key);
                let presence_only = self.overflowed.0.contains(&source);
                self.columns.push(LocalColumn::new(source, self.rows, presence_only));
                self.field_at.insert(key, self.columns.len() - 1);
                self.columns.len() - 1
            }
        };
        &mut self.columns[at]
    }

    /// Add the next line of the block.
    pub fn push(&mut self, facts: &LineFacts) {
        let row = self.rows;
        let role = if facts.has_time {
            Role::Parent
        } else if self.seen_parent {
            Role::Continuation
        } else {
            Role::Unsettled
        };
        self.roles.push(role);
        self.seen_parent |= role == Role::Parent;

        // A Continuation Line is all message: its values are its Parent
        // Line's, only its Stream is its own, and it has no field.
        if role == Role::Continuation {
            for col in &mut self.columns {
                let value = if col.source.inherits() { None } else { facts.value(&col.source, false) };
                col.record(value);
            }
            if let Some(stream) = facts.stream {
                self.max_chars.stream = self.max_chars.stream.max(stream.as_str().len());
            }
            self.timestamps.push(f64::NAN);
            self.rows += 1;
            return;
        }
        let settled = role == Role::Parent;

        // Where each field column first shows (`entry.fields()` order).
        let places = facts.fields.iter().chain(&facts.extra).enumerate();
        for (place, (key, _)) in places {
            let col = self.field_column(*key);
            let first = if settled { &mut col.first } else { &mut col.first_if_parents };
            first.get_or_insert((row, place));
        }
        for col in &mut self.columns {
            col.record(facts.value(&col.source, true));
        }
        let kv = if settled { &mut self.kv } else { &mut self.kv_if_parents };
        for key in &facts.kv_keys {
            *kv.entry(*key).or_default() += 1;
        }
        let max = if settled { &mut self.max_chars } else { &mut self.max_if_parents };
        max.timestamp = max.timestamp.max(facts.time_str.chars().count());
        max.level = max.level.max(facts.level_text.chars().count());
        max.module = max.module.max(facts.tag.chars().count());
        if let Some(source) = facts.source {
            max.source = max.source.max(source.chars().count());
        }
        if let Some(stream) = facts.stream {
            self.max_chars.stream = self.max_chars.stream.max(stream.as_str().len());
        }
        let all_fields: Vec<(FieldKey, &str)> = facts.fields.iter().chain(&facts.extra).copied().collect();
        add_lengths(if settled { &mut self.max_chars } else { &mut self.max_if_parents }, &all_fields);

        match facts.timestamp {
            Some(ts) => {
                self.timestamps.push(ts);
                if self.min_entry.as_ref().is_none_or(|(t, _)| ts < *t) {
                    self.min_entry = Some((ts, facts.time_str.to_string()));
                }
                if self.max_entry.as_ref().is_none_or(|(t, _)| ts > *t) {
                    self.max_entry = Some((ts, facts.time_str.to_string()));
                }
            }
            None => self.timestamps.push(f64::NAN),
        }
        self.rows += 1;
    }

    /// The Unsettled lines settled: whether they are Parent Lines.
    fn parents_on_top(&self, continues: bool) -> bool {
        !continues && self.roles.contains(&Role::Unsettled)
    }
}

impl ColumnIndex {
    /// Add the lines of a block: `local` its values in this column (`None`
    /// when no line of it has the column), `cont` whether each line is a
    /// Continuation Line and `parent_rows` the row of each line's Parent
    /// Line (its own row for a Parent Line).
    fn push_chunk(&mut self, local: Option<&LocalColumn>, cont: &[bool], parent_rows: &[usize]) {
        let ids = local.map(|l| &l.ids[..]);
        let mut remap: Vec<Option<u16>> = vec![None; local.map_or(0, |l| l.spellings.len())];
        let mut uses: Vec<usize> = vec![0; remap.len()];
        for (j, &is_cont) in cont.iter().enumerate() {
            let id = ids.map_or(MISSING, |ids| ids[j]);
            self.rows += 1;
            if !is_cont && id != MISSING {
                self.covered += 1;
            }
            if self.overflow {
                continue;
            }
            if is_cont && self.source.inherits() {
                let inherited = self.row_ids.get(parent_rows[j]).unwrap_or(MISSING);
                self.row_ids.push(inherited);
                continue;
            }
            if id == MISSING {
                self.row_ids.push(MISSING);
                continue;
            }
            debug_assert_ne!(id, PRESENT, "a column read for presence only has overflowed");
            let global = match remap[id as usize] {
                Some(global) => global,
                None => {
                    let (raw, folded) = &local.expect("ids come from a local column").spellings[id as usize];
                    let global = match self.spellings.get(raw) {
                        Some(&(global, _)) => global,
                        None => {
                            let global = match self.folded_ids.get(folded) {
                                Some(&global) => global,
                                None => {
                                    if self.values.len() >= super::MAX_DISTINCT {
                                        self.mark_overflow();
                                        continue;
                                    }
                                    let global = self.values.len() as u16;
                                    self.values.push(super::ColumnValue {
                                        display: raw.clone(),
                                        folded: folded.clone(),
                                        count: 0,
                                    });
                                    self.folded_ids.insert(folded.clone(), global);
                                    global
                                }
                            };
                            self.spellings.insert(raw.clone(), (global, 0));
                            global
                        }
                    };
                    remap[id as usize] = Some(global);
                    global
                }
            };
            self.values[global as usize].count += 1;
            uses[id as usize] += 1;
            self.row_ids.push(global);
        }
        if self.overflow {
            return;
        }
        if let Some(local) = local {
            for (id, n) in uses.into_iter().enumerate().filter(|(_, n)| *n > 0) {
                if let Some(entry) = self.spellings.get_mut(&local.spellings[id].0) {
                    entry.1 += n;
                }
            }
        }
    }
}

/// The columns that overflowed: a worker reading the next block keeps only
/// whether its lines have a value in them.
#[derive(Debug, Clone, Default)]
pub struct Overflowed(HashSet<Source>);

impl FilterIndex {
    pub fn overflowed(&self) -> Overflowed {
        Overflowed(self.columns.iter().filter(|c| c.overflow).map(|c| c.source.clone()).collect())
    }

    /// Add a block of lines read into `chunk`, in file order. `continues`
    /// tells whether a line above the block has a time of its own, so the
    /// lines at its top without one are Continuation Lines.
    pub fn append_chunk(&mut self, chunk: ChunkColumns, continues: bool) {
        let n = chunk.rows;
        if n == 0 {
            return;
        }
        if self.columns.is_empty() && self.rows == 0 {
            self.init_fixed_columns();
        }
        let parents_on_top = chunk.parents_on_top(continues);
        let start = self.rows;
        let mut cont = Vec::with_capacity(n);
        let mut parent_rows = Vec::with_capacity(n);
        for (j, role) in chunk.roles.iter().enumerate() {
            let is_cont = match role {
                Role::Parent => false,
                Role::Continuation => true,
                Role::Unsettled => !parents_on_top,
            };
            if !is_cont {
                self.parent_row = start + j;
                self.parent_lines += 1;
            }
            cont.push(is_cont);
            parent_rows.push(self.parent_row);
        }
        let mut add_kv = |kv: HashMap<FieldKey, usize>| {
            for (key, count) in kv {
                *self.kv_counts.entry(key).or_default() += count;
            }
        };
        add_kv(chunk.kv);
        if parents_on_top {
            add_kv(chunk.kv_if_parents);
        }

        // New field columns, in the order their first line shows them.
        let mut fresh: Vec<((usize, usize), usize)> = Vec::new();
        for (at, col) in chunk.columns.iter().enumerate() {
            let Source::Field(key) = col.source else { continue };
            let first = match (col.first, col.first_if_parents.filter(|_| parents_on_top)) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            };
            if let Some(first) = first {
                if !self.field_cols.contains(&key) {
                    fresh.push((first, at));
                }
            }
        }
        fresh.sort();
        for (_, at) in fresh {
            let source = chunk.columns[at].source.clone();
            if let Source::Field(key) = source {
                self.field_cols.insert(key);
            }
            self.columns.push(ColumnIndex::new(source, start));
        }

        let local_of: HashMap<&Source, usize> =
            chunk.columns.iter().enumerate().map(|(at, c)| (&c.source, at)).collect();
        for col in &mut self.columns {
            let local = local_of.get(&col.source).map(|&at| &chunk.columns[at]);
            col.push_chunk(local, &cont, &parent_rows);
        }

        // No time is kept per row: a file's index reads them from its
        // `LineMeta` (lean-view-memory, decision 8).
        for (j, ts) in chunk.timestamps.iter().enumerate() {
            if !ts.is_nan() {
                self.timed_lines += 1;
                if self.first_ts.is_none() {
                    self.first_ts = Some(*ts);
                    self.first_ts_row = Some(start + j);
                }
            }
        }
        if let Some((ts, text)) = chunk.min_entry {
            if self.min_entry.as_ref().is_none_or(|(t, _)| ts < *t) {
                self.min_entry = Some((ts, text));
            }
        }
        if let Some((ts, text)) = chunk.max_entry {
            if self.max_entry.as_ref().is_none_or(|(t, _)| ts > *t) {
                self.max_entry = Some((ts, text));
            }
        }
        self.rows += n;
        for col in &mut self.columns {
            col.finalize_display();
        }
    }
}

impl ChunkColumns {
    /// The longest texts of the block, once it is settled.
    pub fn settled_max_chars(&self, continues: bool) -> FieldMaxChars {
        let mut max = self.max_chars.clone();
        if self.parents_on_top(continues) {
            max.merge(&self.max_if_parents);
        }
        max
    }
}
