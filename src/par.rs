//! Scans over every row of a log, split across the cores. A log of a million
//! lines takes tens of milliseconds to scan on one core, which is felt on
//! every keystroke; small logs are scanned in place.

use std::thread;

/// Below this many rows a scan is not worth the threads.
const MIN_PARALLEL: usize = 20_000;

fn chunk_len(len: usize) -> usize {
    let cores = thread::available_parallelism().map_or(1, |n| n.get());
    len.div_ceil(cores.max(1)).max(MIN_PARALLEL / 4)
}

/// `f(row)` for every row in `0..len`, in order.
pub fn map_rows<T: Send>(len: usize, f: impl Fn(usize) -> T + Sync) -> Vec<T> {
    if len < MIN_PARALLEL {
        return (0..len).map(f).collect();
    }
    let chunk = chunk_len(len);
    let f = &f;
    thread::scope(|s| {
        let parts: Vec<_> = (0..len)
            .step_by(chunk)
            .map(|start| s.spawn(move || (start..(start + chunk).min(len)).map(f).collect::<Vec<T>>()))
            .collect();
        let mut out = Vec::with_capacity(len);
        for part in parts {
            out.extend(part.join().expect("row scan panicked"));
        }
        out
    })
}

/// The items of `items` for which `keep` holds, in order.
pub fn filter<T: Copy + Send + Sync>(items: &[T], keep: impl Fn(T) -> bool + Sync) -> Vec<T> {
    if items.len() < MIN_PARALLEL {
        return items.iter().copied().filter(|&i| keep(i)).collect();
    }
    let chunk = chunk_len(items.len());
    let keep = &keep;
    thread::scope(|s| {
        let parts: Vec<_> = items
            .chunks(chunk)
            .map(|part| s.spawn(move || part.iter().copied().filter(|&i| keep(i)).collect::<Vec<T>>()))
            .collect();
        let mut out = Vec::new();
        for part in parts {
            out.extend(part.join().expect("row scan panicked"));
        }
        out
    })
}

/// How many of `items` `keep` holds for.
pub fn count<T: Copy + Send + Sync>(items: &[T], keep: impl Fn(T) -> bool + Sync) -> usize {
    if items.len() < MIN_PARALLEL {
        return items.iter().filter(|&&i| keep(i)).count();
    }
    let chunk = chunk_len(items.len());
    let keep = &keep;
    thread::scope(|s| {
        let parts: Vec<_> = items
            .chunks(chunk)
            .map(|part| s.spawn(move || part.iter().filter(|&&i| keep(i)).count()))
            .collect();
        parts.into_iter().map(|p| p.join().expect("row scan panicked")).sum()
    })
}
