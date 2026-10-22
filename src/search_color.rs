//! Colors of the Phrases of a Search Expression: each distinct Phrase is
//! painted in a color of its own, however `||`, `&&` and parentheses join it
//! to the others. The hue is generated from the case-folded Phrase, so the
//! same Phrase has the same color in every tab and every session; nothing is
//! stored. Lightness and saturation are fixed per theme so text stays
//! readable.

use crate::palette::Tone;

/// Hues closer than this (in degrees) are pushed apart.
pub const MIN_HUE_GAP: f32 = 30.0;
/// Step of the search for a free hue.
const HUE_STEP: f32 = 5.0;

/// Opacity of the Phrase color behind an ordinary Match.
pub const MATCH_OPACITY: f32 = 0.4;
/// Opacity of the Phrase color behind the Current Match (it also gets the outline).
pub const CURRENT_MATCH_OPACITY: f32 = 0.7;
/// Opacity of the single shared Match color (Phrase colors off).
pub const SHARED_SEARCH_OPACITY: f32 = 0.55;
/// Opacity of the Focus Find color (Phrase colors off).
pub const FIND_OPACITY: f32 = 0.6;
/// Opacity of the minimap row tint behind an ordinary Match.
pub const MINIMAP_TINT_OPACITY: f32 = 0.3;
/// Opacity of the minimap row tint behind the Current Match.
pub const MINIMAP_CURRENT_TINT_OPACITY: f32 = 0.6;
/// Opacity of a minimap tick stripe of an ordinary Match.
pub const MINIMAP_TICK_OPACITY: f32 = 0.8;
/// Opacity of a minimap tick stripe of the Current Match.
pub const MINIMAP_CURRENT_TICK_OPACITY: f32 = 1.0;

/// The hue (degrees, `0.0..360.0`) a Phrase hashes to. `key` is the Phrase,
/// already case-folded (see `Matcher::phrase_keys`).
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

/// The hue of every distinct Phrase, in order of appearance. A Phrase takes
/// the hue of its key unless an earlier one is within `MIN_HUE_GAP`; then the
/// next free hue clockwise is taken. When no hue is free (past about 12
/// Phrases) the hue farthest from the others is taken: closeness is accepted.
pub fn phrase_hues(keys: &[&str]) -> Vec<f32> {
    phrase_hues_following(keys, &[])
}

/// Like `phrase_hues`, for an expression that must agree with another one
/// (Focus Find with Search): a Phrase also in `known` takes the hue it has
/// there, and only the rest are separated, from those and from each other.
/// With `known` empty this is `phrase_hues`.
pub fn phrase_hues_following(keys: &[&str], known: &[&str]) -> Vec<f32> {
    let known_hues = phrase_hues_of(known, &[]);
    let fixed: Vec<Option<f32>> = keys
        .iter()
        .map(|k| known.iter().position(|n| n == k).map(|i| known_hues[i]))
        .collect();
    phrase_hues_of(keys, &fixed)
}

/// Hues of `keys`; `fixed[i]`, when present and `Some`, is the hue of key `i`
/// already. Fixed hues are placed first, the others follow in order.
fn phrase_hues_of(keys: &[&str], fixed: &[Option<f32>]) -> Vec<f32> {
    let mut hues: Vec<Option<f32>> = (0..keys.len()).map(|i| fixed.get(i).copied().flatten()).collect();
    for i in 0..keys.len() {
        if hues[i].is_some() {
            continue;
        }
        let placed: Vec<f32> = hues.iter().flatten().copied().collect();
        hues[i] = Some(free_hue(hue_for_key(keys[i]), &placed));
    }
    hues.into_iter().flatten().collect()
}

/// `base`, or the next hue clockwise at least `MIN_HUE_GAP` from every hue in
/// `placed`, or the farthest one when none is.
fn free_hue(base: f32, placed: &[f32]) -> f32 {
    let steps = (360.0 / HUE_STEP) as usize;
    let gap_of = |hue: f32| placed.iter().map(|&h| hue_distance(h, hue)).fold(f32::MAX, f32::min);
    let mut best = (base, gap_of(base));
    if best.1 < MIN_HUE_GAP {
        for k in 1..steps {
            let hue = (base + k as f32 * HUE_STEP) % 360.0;
            let gap = gap_of(hue);
            if gap >= MIN_HUE_GAP {
                return hue;
            }
            if gap > best.1 {
                best = (hue, gap);
            }
        }
    }
    best.0
}

/// The Phrase color for a hue, as `0xRRGGBB`, with saturation and lightness
/// fixed per theme. Paint it at `MATCH_OPACITY` / `CURRENT_MATCH_OPACITY`
/// behind unchanged text.
pub fn phrase_rgb(hue: f32, tone: Tone) -> u32 {
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
