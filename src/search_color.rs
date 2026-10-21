//! Color Groups of a Search Expression: each top-level `||` branch is painted
//! in a color of its own. The hue is generated from the group's first Phrase,
//! so the same keyword has the same color in every tab and every session;
//! nothing is stored. Lightness and saturation are fixed per theme so text
//! stays readable.

use crate::palette::Tone;

/// Hues closer than this (in degrees) are pushed apart.
pub const MIN_HUE_GAP: f32 = 30.0;
/// Step of the search for a free hue.
const HUE_STEP: f32 = 5.0;

/// Opacity of the group color behind an ordinary Match.
pub const MATCH_OPACITY: f32 = 0.4;
/// Opacity of the group color behind the Current Match (it also gets the outline).
pub const CURRENT_MATCH_OPACITY: f32 = 0.7;

/// The hue (degrees, `0.0..360.0`) a keyword hashes to. `key` is the first
/// Phrase of the group, already case-folded (see `Matcher::group_keys`).
/// FNV-1a over the UTF-8 bytes: fixed forever, never `std`'s hasher.
pub fn hue_for_key(key: &str) -> f32 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in key.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    (h % 3600) as f32 / 10.0
}

/// Shortest distance between two hues around the color wheel, in degrees.
pub fn hue_distance(a: f32, b: f32) -> f32 {
    let d = (a - b).abs() % 360.0;
    d.min(360.0 - d)
}

/// The hue of every group, in order of appearance. A group takes the hue of
/// its key unless an earlier group is within `MIN_HUE_GAP`; then the next
/// free hue clockwise is taken. When no hue is free (past about 12 groups) the
/// hue farthest from the others is taken: closeness is accepted.
pub fn group_hues(keys: &[&str]) -> Vec<f32> {
    let mut hues: Vec<f32> = Vec::with_capacity(keys.len());
    for key in keys {
        let base = hue_for_key(key);
        let steps = (360.0 / HUE_STEP) as usize;
        let gap_of = |hue: f32| hues.iter().map(|&h| hue_distance(h, hue)).fold(f32::MAX, f32::min);
        let mut best = (base, gap_of(base));
        if best.1 < MIN_HUE_GAP {
            for k in 1..steps {
                let hue = (base + k as f32 * HUE_STEP) % 360.0;
                let gap = gap_of(hue);
                if gap >= MIN_HUE_GAP {
                    best = (hue, gap);
                    break;
                }
                if gap > best.1 {
                    best = (hue, gap);
                }
            }
        }
        hues.push(best.0);
    }
    hues
}

/// The group color for a hue, as `0xRRGGBB`, with saturation and lightness
/// fixed per theme. Paint it at `MATCH_OPACITY` / `CURRENT_MATCH_OPACITY`
/// behind unchanged text.
pub fn group_rgb(hue: f32, tone: Tone) -> u32 {
    let (s, l) = match tone {
        Tone::Light => (0.8, 0.65),
        Tone::Dark => (0.6, 0.30),
    };
    hsl_to_rgb(hue, s, l)
}

fn hsl_to_rgb(hue: f32, s: f32, l: f32) -> u32 {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h = (hue.rem_euclid(360.0)) / 60.0;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let byte = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u32;
    (byte(r) << 16) | (byte(g) << 8) | byte(b)
}
