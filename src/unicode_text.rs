//! Unicode-aware text matching shared by Search and Filter.
//!
//! Matching is case-insensitive per Unicode, compares text in NFC form (so
//! composed and decomposed characters match each other), and never ignores
//! diacritics. Match offsets are byte offsets into the original text.

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::canonical_combining_class;

/// First code point that can be a combining mark; anything below it is a
/// stand-alone character, which lets ASCII/Latin text skip normalization.
const FIRST_COMBINING: char = '\u{300}';

fn is_mark(c: char) -> bool {
    c >= FIRST_COMBINING && canonical_combining_class(c) != 0
}

/// End byte offset of the segment (a character plus its combining marks)
/// that starts at `pos`.
fn segment_end(text: &str, pos: usize) -> usize {
    let mut chars = text[pos..].char_indices();
    let Some((_, first)) = chars.next() else {
        return pos;
    };
    let mut end = pos + first.len_utf8();
    for (i, c) in chars {
        if is_mark(c) {
            end = pos + i + c.len_utf8();
        } else {
            break;
        }
    }
    end
}

fn push_folded(segment: &str, out: &mut Vec<char>) {
    for c in segment.nfc() {
        out.extend(c.to_lowercase());
    }
}

/// Fold `text` for comparison: NFC, then Unicode lowercase.
pub fn fold(text: &str) -> String {
    let mut out = Vec::with_capacity(text.len());
    let mut pos = 0;
    while pos < text.len() {
        let end = segment_end(text, pos);
        push_folded(&text[pos..end], &mut out);
        pos = end;
    }
    out.into_iter().collect()
}

/// A search needle folded once and matched many times.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Needle {
    folded: Vec<char>,
}

impl Needle {
    pub fn new(text: &str) -> Self {
        Self {
            folded: fold(text).chars().collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.folded.is_empty()
    }

    /// If the needle matches `hay` starting exactly at byte `pos`, return the
    /// end byte offset of the match. The match must start and end on
    /// character boundaries (a base character is never matched apart from its
    /// combining marks).
    pub fn match_at(&self, hay: &str, pos: usize) -> Option<usize> {
        if self.folded.is_empty() || pos >= hay.len() || !hay.is_char_boundary(pos) {
            return None;
        }
        let mut idx = 0;
        let mut at = pos;
        let mut scratch: Vec<char> = Vec::new();
        while idx < self.folded.len() {
            if at >= hay.len() {
                return None;
            }
            let end = segment_end(hay, at);
            let segment = &hay[at..end];
            if segment.len() == 1 {
                // Fast path: a lone ASCII character needs no normalization.
                let c = segment.as_bytes()[0].to_ascii_lowercase() as char;
                if self.folded[idx] != c {
                    return None;
                }
                idx += 1;
            } else {
                scratch.clear();
                push_folded(segment, &mut scratch);
                if idx + scratch.len() > self.folded.len()
                    || self.folded[idx..idx + scratch.len()] != scratch[..]
                {
                    return None;
                }
                idx += scratch.len();
            }
            at = end;
        }
        Some(at)
    }

    /// First match at or after byte `from`.
    pub fn find_from(&self, hay: &str, from: usize) -> Option<(usize, usize)> {
        if self.folded.is_empty() {
            return None;
        }
        let first = self.folded[0];
        let bytes = hay.as_bytes();
        let mut pos = from;
        while pos < hay.len() {
            // Cheap rejection: an ASCII byte that cannot start a match.
            if first.is_ascii() && bytes[pos] < 0x80 && bytes[pos].to_ascii_lowercase() as char != first {
                pos += 1;
                continue;
            }
            if let Some(end) = self.match_at(hay, pos) {
                return Some((pos, end));
            }
            pos = segment_end(hay, pos);
        }
        None
    }

    /// Whether the needle occurs anywhere in `hay`.
    pub fn is_in(&self, hay: &str) -> bool {
        self.find_from(hay, 0).is_some()
    }

    /// All non-overlapping matches as byte ranges in `hay`.
    pub fn find_all(&self, hay: &str) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        let mut from = 0;
        while let Some((s, e)) = self.find_from(hay, from) {
            out.push((s, e));
            from = e;
        }
        out
    }

    /// Whether `hay` equals the needle as a whole.
    pub fn equals(&self, hay: &str) -> bool {
        self.match_at(hay, 0) == Some(hay.len())
    }
}

/// Case-insensitive, NFC-aware equality.
pub fn eq(a: &str, b: &str) -> bool {
    if a.is_ascii() && b.is_ascii() {
        return a.eq_ignore_ascii_case(b);
    }
    fold(a) == fold(b)
}

/// Case-insensitive, NFC-aware substring test. An empty needle never matches.
pub fn contains(hay: &str, needle: &str) -> bool {
    Needle::new(needle).is_in(hay)
}
