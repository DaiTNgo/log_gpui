//! Value Colors: the text color the app gives each value of an identifier
//! column, worked out from the value itself (the same FNV-1a hash as Phrase
//! colors), so one value has one color in every file and every session. The
//! user cannot change it per column or value, only turn it off app-wide.

use crate::model::{FieldKey, LogEntry};
use crate::palette::{HighlightColor, Tone, contrast_ratio};
use crate::search_color::{MIN_HUE_GAP, hsl_to_rgb, hue_distance, hue_for_key};
use std::collections::{HashMap, HashSet};

/// Pages Value Colors are read on, as the Canvas paints them.
const LIGHT_PAGE: u32 = 0xFFFFFF;
const DARK_PAGE: u32 = 0x1E1E1E;
/// Contrast a Value Color keeps with the page.
pub const MIN_CONTRAST: f32 = 4.5;
/// Step of the search for a hue away from the Text Highlights.
const HUE_STEP: f32 = 5.0;
/// Values sampled per column to tell its kind.
pub const SAMPLE_VALUES: usize = 64;

/// Names of fields that hold a time, a level or the message: never colored.
const FIXED_NAMES: [&str; 11] =
    ["time", "timestamp", "@timestamp", "datetime", "date", "ts", "level", "lvl", "severity", "msg", "message"];

/// The hue of a color, in degrees.
pub fn hue_of(rgb: u32) -> f32 {
    let ch = |shift: u32| ((rgb >> shift) & 0xFF) as f32 / 255.0;
    let (r, g, b) = (ch(16), ch(8), ch(0));
    let max = r.max(g).max(b);
    let d = max - r.min(g).min(b);
    if d <= f32::EPSILON {
        return 0.0;
    }
    let h = if max == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    h * 60.0
}

/// The hues a Value Color keeps away from: those of the Text Highlights.
pub fn hues_to_avoid<'a>(colors: impl Iterator<Item = &'a HighlightColor>, tone: Tone) -> Vec<f32> {
    colors.map(|c| hue_of(c.rgb(tone))).collect()
}

/// `hue`, or the nearest hue at least `MIN_HUE_GAP` from every hue of
/// `avoid`. When none is free, `hue` as it is.
pub fn hue_avoiding(hue: f32, avoid: &[f32]) -> f32 {
    let free = |h: f32| avoid.iter().all(|&a| hue_distance(h, a) >= MIN_HUE_GAP);
    let steps = (180.0 / HUE_STEP) as i32;
    for i in 0..=steps {
        for sign in [1.0, -1.0] {
            let h = (hue + sign * i as f32 * HUE_STEP).rem_euclid(360.0);
            if free(h) {
                return h;
            }
        }
    }
    hue
}

/// A text color of `hue` that reads on the page of `tone`.
pub fn text_rgb_for_hue(hue: f32, tone: Tone) -> u32 {
    let (s, mut l, step, page) = match tone {
        Tone::Light => (0.70, 0.42, -0.02, LIGHT_PAGE),
        Tone::Dark => (0.65, 0.66, 0.02, DARK_PAGE),
    };
    let mut rgb = hsl_to_rgb(hue, s, l);
    for _ in 0..20 {
        if contrast_ratio(rgb, page) >= MIN_CONTRAST {
            break;
        }
        l += step;
        rgb = hsl_to_rgb(hue, s, l);
    }
    rgb
}

/// The Value Color of `value`, with letter case ignored.
pub fn value_rgb(value: &str, tone: Tone, avoid: &[f32]) -> u32 {
    let hue = hue_for_key(&crate::unicode_text::fold(value));
    text_rgb_for_hue(hue_avoiding(hue, avoid), tone)
}

/// Whether a value is a number, a boolean or JSON: kinds that get no color.
pub fn is_plain_value(value: &str) -> bool {
    let v = value.trim();
    v.parse::<f64>().is_ok()
        || v.eq_ignore_ascii_case("true")
        || v.eq_ignore_ascii_case("false")
        || v.starts_with('{')
        || v.starts_with('[')
}

/// Tells which Field Columns get a Value Color from a sample of their
/// values: a column whose values are all numbers, booleans or JSON gets none,
/// and neither does a field named for a time, a level or the message.
#[derive(Debug, Clone, Default)]
pub struct FieldSampler {
    /// Values seen and whether one of them was text.
    seen: HashMap<FieldKey, (usize, bool)>,
}

impl FieldSampler {
    /// Look at the values of `fields` in one line.
    pub fn add(&mut self, entry: &LogEntry, fields: &[FieldKey]) {
        for &key in fields {
            let Some(range) = entry.span(key.as_str()) else { continue };
            let Some(value) = entry.raw_line.get(range) else { continue };
            if value.is_empty() {
                continue;
            }
            let (count, text) = self.seen.entry(key).or_default();
            if *count >= SAMPLE_VALUES {
                continue;
            }
            *count += 1;
            *text |= !is_plain_value(value);
        }
    }

    /// Whether every one of `fields` has its full sample.
    pub fn is_full(&self, fields: &[FieldKey]) -> bool {
        fields.iter().all(|k| self.seen.get(k).is_some_and(|(n, _)| *n >= SAMPLE_VALUES))
    }

    /// The fields that get a Value Color.
    pub fn colored(&self) -> HashSet<FieldKey> {
        self.seen
            .iter()
            .filter(|(k, (_, text))| *text && !FIXED_NAMES.iter().any(|n| n.eq_ignore_ascii_case(k.as_str())))
            .map(|(k, _)| *k)
            .collect()
    }
}

/// What painting needs to give a cell its Value Color.
#[derive(Debug, Clone)]
pub struct ValueColors {
    /// Field Columns that get a Value Color. The Module column always does.
    pub fields: HashSet<FieldKey>,
    /// Hues to keep away from (the Text Highlights').
    pub avoid: Vec<f32>,
    pub tone: Tone,
}

impl ValueColors {
    /// The Value Color of `value` shown in the Field Column `field`, or in
    /// the Module column for `None`.
    pub fn rgb(&self, field: Option<FieldKey>, value: &str) -> Option<u32> {
        if value.trim().is_empty() || field.is_some_and(|k| !self.fields.contains(&k)) {
            return None;
        }
        Some(value_rgb(value, self.tone, &self.avoid))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_value_same_color_ignoring_case() {
        assert_eq!(value_rgb("T-12", Tone::Light, &[]), value_rgb("T-12", Tone::Light, &[]));
        assert_eq!(value_rgb("API-1", Tone::Dark, &[]), value_rgb("api-1", Tone::Dark, &[]));
    }

    #[test]
    fn colors_are_stable_across_runs() {
        // FNV-1a is fixed: this hue must never change.
        assert_eq!(hue_for_key("t-12"), hue_for_key(&crate::unicode_text::fold("T-12")));
        let a = value_rgb("T-12", Tone::Light, &[]);
        assert_eq!(a, text_rgb_for_hue(hue_for_key("t-12"), Tone::Light));
    }

    #[test]
    fn every_hue_reads_on_both_pages() {
        for h in (0..360).step_by(5) {
            let light = text_rgb_for_hue(h as f32, Tone::Light);
            let dark = text_rgb_for_hue(h as f32, Tone::Dark);
            assert!(contrast_ratio(light, LIGHT_PAGE) >= MIN_CONTRAST, "light hue {h}");
            assert!(contrast_ratio(dark, DARK_PAGE) >= MIN_CONTRAST, "dark hue {h}");
        }
    }

    #[test]
    fn keeps_away_from_text_highlight_hues() {
        let hue = hue_for_key("t-12");
        let moved = hue_avoiding(hue, &[hue]);
        assert!(hue_distance(moved, hue) >= MIN_HUE_GAP);
        assert_eq!(hue_avoiding(hue, &[(hue + 180.0) % 360.0]), hue);
    }

    #[test]
    fn hue_of_palette_like_colors() {
        assert!((hue_of(0xFF0000) - 0.0).abs() < 0.5);
        assert!((hue_of(0x00FF00) - 120.0).abs() < 0.5);
        assert!((hue_of(0x0000FF) - 240.0).abs() < 0.5);
    }

    #[test]
    fn plain_values() {
        for v in ["132", "-4.5", "1e3", "true", "FALSE", "{\"a\":1}", "[1,2]"] {
            assert!(is_plain_value(v), "{v}");
        }
        for v in ["T-12", "db-1", "req-91bc", "10.0.0.1"] {
            assert!(!is_plain_value(v), "{v}");
        }
    }

    fn entry_with(fields: &[(&str, &str)]) -> LogEntry {
        let mut line = String::new();
        let mut spans = std::collections::HashMap::new();
        for (k, v) in fields {
            let start = line.len();
            line.push_str(v);
            spans.insert(k.to_string(), start..line.len());
            line.push(' ');
        }
        let mut e = LogEntry::new(1, 0.0, String::new(), String::new(), crate::model::LogLevel::Info, line.clone(), line);
        e.spans = crate::model::Spans::from_map(&spans);
        e
    }

    #[test]
    fn number_boolean_json_and_fixed_columns_get_no_color() {
        let keys: Vec<FieldKey> =
            ["thread", "duration", "ok", "body", "time"].iter().map(|k| FieldKey::new(k)).collect();
        let mut s = FieldSampler::default();
        s.add(&entry_with(&[("thread", "T-12"), ("duration", "132"), ("ok", "true"), ("body", "{}"), ("time", "10:00")]), &keys);
        s.add(&entry_with(&[("thread", "T-7"), ("duration", "9"), ("ok", "false"), ("body", "[]"), ("time", "10:01")]), &keys);
        let colored = s.colored();
        assert_eq!(colored, HashSet::from([FieldKey::new("thread")]));
    }

    #[test]
    fn a_column_with_any_text_value_is_colored() {
        let keys = vec![FieldKey::new("user")];
        let mut s = FieldSampler::default();
        s.add(&entry_with(&[("user", "42")]), &keys);
        s.add(&entry_with(&[("user", "alice")]), &keys);
        assert!(s.colored().contains(&FieldKey::new("user")));
    }

    #[test]
    fn module_always_colored_fields_only_when_listed() {
        let vc = ValueColors { fields: HashSet::from([FieldKey::new("thread")]), avoid: vec![], tone: Tone::Light };
        assert!(vc.rgb(None, "pool").is_some());
        assert!(vc.rgb(Some(FieldKey::new("thread")), "T-12").is_some());
        assert!(vc.rgb(Some(FieldKey::new("duration")), "132").is_none());
        assert!(vc.rgb(None, "  ").is_none());
    }
}
