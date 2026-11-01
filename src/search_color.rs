//! Colors of the Phrases of a Search Expression: each distinct Phrase is
//! painted in a color of its own, however `||`, `&&` and parentheses join it
//! to the others. The first Phrases take the vivid `FIXED_HUES` in order of
//! appearance; past those, the hue is generated from the case-folded Phrase,
//! so it is the same in every tab and every session. Only a hue the user
//! chose for a Phrase is stored. Lightness and saturation are fixed per theme so text stays
//! readable.

use crate::palette::Tone;

/// Hues closer than this (in degrees) are pushed apart.
pub const MIN_HUE_GAP: f32 = 30.0;
/// The first Phrases of an expression take these hues in order: vivid and at
/// least `MIN_HUE_GAP` apart. Only Phrases past them take a hue from their key.
pub const FIXED_HUES: [f32; 8] = [210.0, 25.0, 140.0, 320.0, 55.0, 175.0, 355.0, 270.0];
/// Step of the search for a free hue.
const HUE_STEP: f32 = 5.0;

/// Opacity of the Phrase color behind an ordinary Match.
pub const MATCH_OPACITY: f32 = 0.4;
/// Opacity of the Phrase color behind the Current Match (it also gets the outline).
pub const CURRENT_MATCH_OPACITY: f32 = 0.7;
/// Opacity of a color the user picked behind an ordinary Match: stronger than
/// a generated one, so the color reads as the one chosen.
pub const CUSTOM_MATCH_OPACITY: f32 = 0.85;
/// Opacity of a picked color behind the Current Match (it also gets the outline).
pub const CUSTOM_CURRENT_MATCH_OPACITY: f32 = 1.0;
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

/// Hues the user chose for Phrases, by case-folded key. They replace the
/// generated hue everywhere a Phrase is painted; the app keeps them in the
/// workspace state and loads them here at start.
static HUE_OVERRIDES: std::sync::RwLock<std::collections::BTreeMap<String, f32>> =
    std::sync::RwLock::new(std::collections::BTreeMap::new());

/// Replace every chosen hue.
pub fn set_hue_overrides(hues: std::collections::BTreeMap<String, f32>) {
    if let Ok(mut m) = HUE_OVERRIDES.write() {
        *m = hues;
    }
}

/// Exact colors (`0xRRGGBB`) the user picked for Phrases with the color
/// picker, by case-folded key. They are painted as they are, in every theme,
/// and win over a chosen hue.
static RGB_OVERRIDES: std::sync::RwLock<std::collections::BTreeMap<String, u32>> =
    std::sync::RwLock::new(std::collections::BTreeMap::new());

/// Replace every picked color.
pub fn set_rgb_overrides(colors: std::collections::BTreeMap<String, u32>) {
    if let Ok(mut m) = RGB_OVERRIDES.write() {
        *m = colors;
    }
}

/// What the user is pointing at in the color menu of a Phrase. Nothing is stored.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Preview {
    /// The generated color (what Reset would give).
    Generated,
    Hue(f32),
    Rgb(u32),
}

static PREVIEW: std::sync::RwLock<Option<(String, Preview)>> = std::sync::RwLock::new(None);

/// Show (or with `None`, stop showing) a preview.
pub fn set_preview(preview: Option<(String, Preview)>) {
    if let Ok(mut p) = PREVIEW.write() {
        *p = preview;
    }
}

/// The preview being shown, with its Phrase key.
pub fn preview() -> Option<(String, Preview)> {
    PREVIEW.read().ok()?.clone()
}

/// The hue the user chose for a Phrase and kept, if any.
pub fn stored_hue_override(key: &str) -> Option<f32> {
    HUE_OVERRIDES.read().ok()?.get(key).copied()
}

/// The exact color the user picked for a Phrase and kept, if any.
pub fn stored_rgb_override(key: &str) -> Option<u32> {
    RGB_OVERRIDES.read().ok()?.get(key).copied()
}

/// The preview of `key`, when it is the Phrase being previewed.
fn preview_of(key: &str) -> Option<Preview> {
    PREVIEW.read().ok()?.as_ref().filter(|(k, _)| k == key).map(|(_, p)| *p)
}

/// The hue to paint a Phrase with instead of the generated one: the one being
/// previewed, else the one the user kept.
pub fn hue_override(key: &str) -> Option<f32> {
    match preview_of(key) {
        Some(Preview::Hue(h)) => Some(h),
        Some(_) => None,
        None => stored_hue_override(key),
    }
}

/// The exact color to paint a Phrase with, if one is previewed or kept.
pub fn rgb_override(key: &str) -> Option<u32> {
    match preview_of(key) {
        Some(Preview::Rgb(c)) => Some(c),
        Some(_) => None,
        None => stored_rgb_override(key),
    }
}

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
    let mut hues: Vec<Option<f32>> = (0..keys.len())
        .map(|i| fixed.get(i).copied().flatten().or_else(|| hue_override(keys[i])))
        .collect();
    for i in 0..keys.len() {
        if hues[i].is_some() {
            continue;
        }
        let placed: Vec<f32> = hues.iter().flatten().copied().collect();
        let fixed_free = FIXED_HUES.iter().copied().find(|&h| placed.iter().all(|&p| hue_distance(p, h) >= MIN_HUE_GAP));
        hues[i] = Some(fixed_free.unwrap_or_else(|| free_hue(hue_for_key(keys[i]), &placed)));
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
        Tone::Dark => (0.7, dark_lightness(hue, 0.7)),
    };
    hsl_to_rgb(hue, s, l)
}

/// Page and text colors of the dark theme the Phrase colors are tuned for.
const DARK_PAGE: u32 = 0x1e1e1e;
const DARK_TEXT: u32 = 0xd4d4d4;

/// The brightest lightness (down to 0.20) at which text stays readable on the
/// hue at `CURRENT_MATCH_OPACITY`: yellows and greens are brighter than blues
/// and purples at the same lightness, so they get less.
fn dark_lightness(hue: f32, s: f32) -> f32 {
    let lum = |rgb: u32| {
        let ch = |shift: u32| {
            let v = ((rgb >> shift) & 0xff) as f32 / 255.0;
            if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * ch(16) + 0.7152 * ch(8) + 0.0722 * ch(0)
    };
    let blend = |c: u32| {
        let mix = |shift: u32| {
            let f = ((c >> shift) & 0xff) as f32;
            let b = ((DARK_PAGE >> shift) & 0xff) as f32;
            ((f * CURRENT_MATCH_OPACITY + b * (1.0 - CURRENT_MATCH_OPACITY)).round() as u32) << shift
        };
        mix(16) | mix(8) | mix(0)
    };
    let text = lum(DARK_TEXT);
    (20..=45)
        .rev()
        .map(|l| l as f32 / 100.0)
        .find(|&l| (text + 0.05) / (lum(blend(hsl_to_rgb(hue, s, l))) + 0.05) >= 4.6)
        .unwrap_or(0.20)
}

pub fn hsl_to_rgb(hue: f32, s: f32, l: f32) -> u32 {
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
