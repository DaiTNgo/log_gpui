//! Load More for a local file: the lines above the oldest line the app holds,
//! found by that line's number in the file.
use super::backward::{BackwardLines, ReadAt};
use super::source::SourceEvent;
use crate::workspace::log_space::line_hash;
use async_channel::Sender;
use std::fs::File;
use std::path::Path;

const BLOCK: usize = 1 << 20;

/// Where line `number` (1-based) starts: after `number - 1` newlines.
fn offset_of_line(file: &mut File, number: usize) -> Option<u64> {
    let mut wanted = number.checked_sub(1)?;
    let mut offset = 0u64;
    let mut buf = vec![0u8; BLOCK];
    if wanted == 0 {
        return Some(0);
    }
    loop {
        let n = file.read_at(offset, &mut buf).ok()?;
        if n == 0 {
            return None;
        }
        for (i, b) in buf[..n].iter().enumerate() {
            if *b == b'\n' {
                wanted -= 1;
                if wanted == 0 {
                    return Some(offset + i as u64 + 1);
                }
            }
        }
        offset += n as u64;
    }
}

/// The text of the line that starts at `offset`.
fn line_at(file: &mut File, offset: u64) -> Option<String> {
    let mut buf = vec![0u8; 64 * 1024];
    let n = file.read_at(offset, &mut buf).ok()?;
    let end = buf[..n].iter().position(|b| *b == b'\n').unwrap_or(n);
    Some(super::splitter::decode_line(&buf[..end]))
}

/// Read `lines` lines above line `first_line_no` and send them as `Older`,
/// or say they cannot be read (the file changed under the numbers).
pub fn send_older(
    path: &Path,
    first_line_no: Option<usize>,
    first_hash: Option<u64>,
    lines: usize,
    events: &Sender<SourceEvent>,
) {
    let unavailable = || {
        let _ = events.try_send(SourceEvent::OlderUnavailable);
    };
    let Some(number) = first_line_no else { return unavailable() };
    let Ok(mut file) = File::open(path) else { return unavailable() };
    let Some(end) = offset_of_line(&mut file, number) else { return unavailable() };
    if let Some(expected) = first_hash {
        if line_at(&mut file, end).map(|l| line_hash(&l)) != Some(expected) {
            return unavailable();
        }
    }
    let mut backward = BackwardLines::new(end, BLOCK);
    while let Some((offset, len)) = backward.want_block(lines) {
        let mut block = vec![0u8; len];
        match file.read_at(offset, &mut block) {
            Ok(n) => block.truncate(n),
            Err(_) => return unavailable(),
        }
        backward.absorb_block(block);
    }
    let (lines, bytes) = backward.take_batch_with_lens(lines);
    let _ = events.try_send(SourceEvent::Older {
        generation: 0,
        lines,
        bytes,
        reached_start: backward.is_done(),
        before_start_time: false,
    });
}
