//! Text Highlights: a stretch of text and a color, kept by the Project. Every
//! place the text appears in a line, ignoring letter case, is painted behind
//! in that color, in every view. A Marker is one place the user found; a Text
//! Highlight helps find every place.

use crate::palette::HighlightColor;
use crate::unicode_text::{Needle, fold};
use serde::{Deserialize, Serialize};
use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextHighlight {
    /// The text as the user selected it.
    pub text: String,
    pub color: HighlightColor,
    /// Grows each time a Text Highlight is made or recolored: where two
    /// overlap, the higher one paints on top.
    #[serde(default)]
    pub seq: u64,
}

impl TextHighlight {
    /// The case-folded text, which tells Text Highlights apart.
    pub fn key(&self) -> String {
        fold(&self.text)
    }
}

/// Whether `text` can become a Text Highlight: one line, with something
/// other than whitespace in it.
pub fn can_highlight(text: &str) -> bool {
    !text.contains('\n') && !text.trim().is_empty()
}

fn next_seq(list: &[TextHighlight]) -> u64 {
    list.iter().map(|h| h.seq).max().map_or(1, |s| s + 1)
}

/// Highlight `text` in `color`. A Text Highlight for the same text (ignoring
/// case) takes the new color instead of a second one being made. Returns
/// false (and changes nothing) for text that cannot be highlighted.
pub fn add_text_highlight(list: &mut Vec<TextHighlight>, text: &str, color: HighlightColor) -> bool {
    if !can_highlight(text) {
        return false;
    }
    let seq = next_seq(list);
    let key = fold(text);
    match list.iter_mut().find(|h| h.key() == key) {
        Some(h) => {
            h.color = color;
            h.seq = seq;
        }
        None => list.push(TextHighlight { text: text.to_string(), color, seq }),
    }
    true
}

/// Give the Text Highlight with `key` another color; it then paints on top.
pub fn recolor_text_highlight(list: &mut [TextHighlight], key: &str, color: HighlightColor) -> bool {
    let seq = next_seq(list);
    match list.iter_mut().find(|h| h.key() == key) {
        Some(h) => {
            h.color = color;
            h.seq = seq;
            true
        }
        None => false,
    }
}

/// Remove the Text Highlight with `key`, and with it every place it paints.
pub fn remove_text_highlight(list: &mut Vec<TextHighlight>, key: &str) -> bool {
    let before = list.len();
    list.retain(|h| h.key() != key);
    list.len() != before
}

/// One painted stretch of a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightRange {
    /// Bytes of the line.
    pub range: Range<usize>,
    pub color: HighlightColor,
    /// Index of the Text Highlight in [`HighlightMatcher`] order.
    pub owner: usize,
}

/// The Text Highlights of a Project ready to be found in lines.
#[derive(Debug, Clone, Default)]
pub struct HighlightMatcher {
    /// Lowest `seq` first.
    entries: Vec<(Needle, String, HighlightColor)>,
}

impl HighlightMatcher {
    pub fn new(list: &[TextHighlight]) -> Self {
        let mut sorted: Vec<&TextHighlight> = list.iter().collect();
        sorted.sort_by_key(|h| h.seq);
        let entries = sorted
            .into_iter()
            .map(|h| (Needle::new(&h.text), h.key(), h.color.clone()))
            .filter(|(n, _, _)| !n.is_empty())
            .collect();
        Self { entries }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The colors of every Text Highlight.
    pub fn colors(&self) -> impl Iterator<Item = &HighlightColor> {
        self.entries.iter().map(|(_, _, c)| c)
    }

    /// This matcher with the Text Highlight `key` in `color`: a preview.
    pub fn with_color(&self, key: &str, color: &HighlightColor) -> Self {
        let mut out = self.clone();
        for (_, k, c) in &mut out.entries {
            if k == key {
                *c = color.clone();
            }
        }
        out
    }

    /// This matcher with `text` also highlighted in `color`, above the others: a preview.
    pub fn with_extra(&self, text: &str, color: &HighlightColor) -> Self {
        let mut out = self.clone();
        let needle = Needle::new(text);
        if !needle.is_empty() {
            out.entries.push((needle, fold(text), color.clone()));
        }
        out
    }

    /// The key of Text Highlight `owner`.
    pub fn key(&self, owner: usize) -> Option<&str> {
        self.entries.get(owner).map(|(_, k, _)| k.as_str())
    }

    /// The painted stretches of `line`: sorted and not overlapping. Where two
    /// Text Highlights cover the same bytes the newer one owns them.
    pub fn ranges(&self, line: &str) -> Vec<HighlightRange> {
        if self.entries.is_empty() {
            return Vec::new();
        }
        let mut found: Vec<(usize, usize, usize)> = Vec::new();
        for (owner, (needle, _, _)) in self.entries.iter().enumerate() {
            found.extend(needle.find_all(line).into_iter().map(|(s, e)| (s, e, owner)));
        }
        if found.is_empty() {
            return Vec::new();
        }
        let mut points: Vec<usize> = found.iter().flat_map(|&(s, e, _)| [s, e]).collect();
        points.sort_unstable();
        points.dedup();
        let mut out: Vec<HighlightRange> = Vec::new();
        for w in points.windows(2) {
            let (start, end) = (w[0], w[1]);
            let owner = found
                .iter()
                .filter(|&&(s, e, _)| s <= start && end <= e)
                .map(|&(_, _, o)| o)
                .max();
            let Some(owner) = owner else { continue };
            match out.last_mut() {
                Some(last) if last.owner == owner && last.range.end == start => last.range.end = end,
                _ => out.push(HighlightRange { range: start..end, color: self.entries[owner].2.clone(), owner }),
            }
        }
        out
    }

    /// The key of the Text Highlight painting byte `byte` of `line`.
    pub fn key_at(&self, line: &str, byte: usize) -> Option<&str> {
        self.ranges(line)
            .into_iter()
            .find(|r| r.range.start <= byte && byte < r.range.end)
            .and_then(|r| self.key(r.owner))
    }

    /// Whether `line` has a place any Text Highlight paints.
    pub fn is_in(&self, line: &str) -> bool {
        self.entries.iter().any(|(n, _, _)| n.is_in(line))
    }
}

/// How many of `lines` hold `text`, ignoring case.
pub fn count_lines<'a>(text: &str, lines: impl Iterator<Item = &'a str>) -> usize {
    let needle = Needle::new(text);
    if needle.is_empty() {
        return 0;
    }
    lines.filter(|l| needle.is_in(l)).count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::{Hue, Shade};

    fn c(hue: Hue) -> HighlightColor {
        HighlightColor::palette(hue, Shade::Medium)
    }

    #[test]
    fn same_text_in_another_case_recolors_instead_of_adding() {
        let mut list = Vec::new();
        assert!(add_text_highlight(&mut list, "req-91bc", c(Hue::Orange)));
        assert!(add_text_highlight(&mut list, "REQ-91BC", c(Hue::Blue)));
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].color, c(Hue::Blue));
        assert_eq!(list[0].text, "req-91bc");
    }

    #[test]
    fn blank_or_multi_line_text_is_refused() {
        let mut list = Vec::new();
        assert!(!add_text_highlight(&mut list, "  ", c(Hue::Red)));
        assert!(!add_text_highlight(&mut list, "a\nb", c(Hue::Red)));
        assert!(list.is_empty());
    }

    #[test]
    fn remove_takes_every_place() {
        let mut list = Vec::new();
        add_text_highlight(&mut list, "conn-7f3a", c(Hue::Purple));
        assert!(remove_text_highlight(&mut list, &fold("CONN-7F3A")));
        assert!(HighlightMatcher::new(&list).ranges("open conn-7f3a conn-7f3a").is_empty());
    }

    #[test]
    fn every_place_ignoring_case() {
        let mut list = Vec::new();
        add_text_highlight(&mut list, "Timeout", c(Hue::Red));
        let m = HighlightMatcher::new(&list);
        let r: Vec<_> = m.ranges("timeout then TIMEOUT").into_iter().map(|r| r.range).collect();
        assert_eq!(r, vec![0..7, 13..20]);
    }

    #[test]
    fn special_characters_are_plain_text() {
        let mut list = Vec::new();
        add_text_highlight(&mut list, "a.b", c(Hue::Red));
        let m = HighlightMatcher::new(&list);
        assert_eq!(m.ranges("axb a.b").into_iter().map(|r| r.range).collect::<Vec<_>>(), vec![4..7]);
    }

    #[test]
    fn diacritics_keep_byte_offsets_of_the_line() {
        let mut list = Vec::new();
        add_text_highlight(&mut list, "lỗi", c(Hue::Red));
        let m = HighlightMatcher::new(&list);
        let line = "có LỖI ở đây";
        let r = m.ranges(line);
        assert_eq!(r.len(), 1);
        assert_eq!(&line[r[0].range.clone()], "LỖI");
    }

    #[test]
    fn newer_highlight_owns_the_overlap() {
        let mut list = Vec::new();
        add_text_highlight(&mut list, "conn", c(Hue::Blue));
        add_text_highlight(&mut list, "conn-7f3a", c(Hue::Red));
        let m = HighlightMatcher::new(&list);
        let r = m.ranges("conn-7f3a conn");
        assert_eq!(r.len(), 2);
        assert_eq!((r[0].range.clone(), r[0].color.clone()), (0..9, c(Hue::Red)));
        assert_eq!((r[1].range.clone(), r[1].color.clone()), (10..14, c(Hue::Blue)));
    }

    #[test]
    fn recolor_puts_it_on_top() {
        let mut list = Vec::new();
        add_text_highlight(&mut list, "conn-7f3a", c(Hue::Red));
        add_text_highlight(&mut list, "conn", c(Hue::Blue));
        assert!(recolor_text_highlight(&mut list, "conn-7f3a", c(Hue::Green)));
        let m = HighlightMatcher::new(&list);
        assert_eq!(m.ranges("conn-7f3a")[0].color, c(Hue::Green));
        assert_eq!(m.ranges("conn-7f3a")[0].range, 0..9);
    }

    #[test]
    fn repeated_text_side_by_side() {
        let mut list = Vec::new();
        add_text_highlight(&mut list, "ab", c(Hue::Red));
        let m = HighlightMatcher::new(&list);
        // Two places next to each other paint as one stretch.
        assert_eq!(m.ranges("xabab").into_iter().map(|r| r.range).collect::<Vec<_>>(), vec![1..5]);
        assert_eq!(m.key_at("xabab", 3), Some("ab"));
        assert_eq!(m.key_at("xabab", 0), None);
    }

    #[test]
    fn counts_lines_holding_the_text() {
        let lines = ["open conn-7f3a", "GET conn-91bc", "CONN-7F3A slow"];
        assert_eq!(count_lines("conn-7f3a", lines.iter().copied()), 2);
    }
}
