//! Highlight Palette: 10 colors × 3 shades shared by Markers and Highlight
//! Rules, plus free colors from the color picker and the Recent row.
//!
//! The palette is a set of tokens with their own values for the light and the
//! dark theme, so the text on a painted background stays readable in both.

use serde::{Deserialize, Serialize};

/// The most recent colors remembered.
pub const MAX_RECENT: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Hue {
    Red,
    Orange,
    Amber,
    Lime,
    Green,
    Teal,
    Cyan,
    Blue,
    Purple,
    Pink,
}

impl Hue {
    pub const ALL: [Hue; 10] = [
        Hue::Red,
        Hue::Orange,
        Hue::Amber,
        Hue::Lime,
        Hue::Green,
        Hue::Teal,
        Hue::Cyan,
        Hue::Blue,
        Hue::Purple,
        Hue::Pink,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Hue::Red => "Red",
            Hue::Orange => "Orange",
            Hue::Amber => "Amber",
            Hue::Lime => "Lime",
            Hue::Green => "Green",
            Hue::Teal => "Teal",
            Hue::Cyan => "Cyan",
            Hue::Blue => "Blue",
            Hue::Purple => "Purple",
            Hue::Pink => "Pink",
        }
    }

    /// Hue angle in degrees.
    pub fn degrees(self) -> f32 {
        match self {
            Hue::Red => 0.0,
            Hue::Orange => 24.0,
            Hue::Amber => 43.0,
            Hue::Lime => 80.0,
            Hue::Green => 140.0,
            Hue::Teal => 172.0,
            Hue::Cyan => 190.0,
            Hue::Blue => 217.0,
            Hue::Purple => 270.0,
            Hue::Pink => 330.0,
        }
    }
}

/// Which hue paints a kind of match; `None` follows the theme's own color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct MatchColors {
    /// Search Expression matches, in the view and in the Focus Pane.
    #[serde(default)]
    pub search: Option<Hue>,
    /// Focus Find matches, in the Focus Pane.
    #[serde(default)]
    pub find: Option<Hue>,
}

impl Hue {
    /// The hue as a bright `(h, s, l)` for painting text matches, `h` in 0..1.
    pub fn vivid_hsl(self) -> (f32, f32, f32) {
        (self.degrees() / 360.0, 0.85, 0.55)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Shade {
    Light,
    Medium,
    Strong,
}

impl Shade {
    pub const ALL: [Shade; 3] = [Shade::Light, Shade::Medium, Shade::Strong];
}

/// Which theme a color is resolved for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Light,
    Dark,
}

/// A color chosen by the user: a palette token or a free `#RRGGBB`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HighlightColor {
    Palette { hue: Hue, shade: Shade },
    Custom(String),
}

impl HighlightColor {
    pub fn palette(hue: Hue, shade: Shade) -> Self {
        HighlightColor::Palette { hue, shade }
    }

    /// A free color from a `#RRGGBB` text; `None` when the text is no color.
    pub fn custom(hex: &str) -> Option<Self> {
        parse_hex(hex).map(|rgb| HighlightColor::Custom(format_hex(rgb)))
    }

    /// The color as `0xRRGGBB` for `tone`.
    pub fn rgb(&self, tone: Tone) -> u32 {
        match self {
            HighlightColor::Palette { hue, shade } => palette_rgb(*hue, *shade, tone),
            HighlightColor::Custom(hex) => parse_hex(hex).unwrap_or(0x808080),
        }
    }

    /// The same color one step lighter or stronger, for the row and cell
    /// paint of a Highlight Rule. A palette color moves to the given shade of
    /// its hue; a free color is mixed toward the page background.
    pub fn with_weight(&self, weight: Weight, tone: Tone) -> u32 {
        match self {
            HighlightColor::Palette { hue, .. } => {
                let shade = match weight {
                    Weight::Row => Shade::Light,
                    Weight::Cell => Shade::Medium,
                };
                palette_rgb(*hue, shade, tone)
            }
            HighlightColor::Custom(_) => {
                let base = self.rgb(tone);
                let page = match tone {
                    Tone::Light => 0xFFFFFF,
                    Tone::Dark => 0x1E1E1E,
                };
                let keep = match weight {
                    Weight::Row => 0.28,
                    Weight::Cell => 0.55,
                };
                mix(base, page, keep)
            }
        }
    }

    /// Light or dark text, whichever reads better on this color.
    pub fn text_tone(&self, tone: Tone) -> Tone {
        text_tone_on(self.rgb(tone))
    }
}

/// How strongly a Highlight Rule paints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weight {
    /// The whole matching row.
    Row,
    /// The cell that meets the condition.
    Cell,
}

/// The color of palette cell `(hue, shade)` for `tone` as `0xRRGGBB`.
///
/// The lightness starts from the shade's value and is moved away from the
/// middle until the text color of the theme reads on it (contrast 4.5), so
/// every cell of every hue keeps its text readable.
pub fn palette_rgb(hue: Hue, shade: Shade, tone: Tone) -> u32 {
    let (s, mut l) = match (tone, shade) {
        (Tone::Light, Shade::Light) => (0.90, 0.89),
        (Tone::Light, Shade::Medium) => (0.85, 0.79),
        (Tone::Light, Shade::Strong) => (0.80, 0.68),
        (Tone::Dark, Shade::Light) => (0.45, 0.20),
        (Tone::Dark, Shade::Medium) => (0.50, 0.27),
        (Tone::Dark, Shade::Strong) => (0.55, 0.36),
    };
    // On the light theme the text is dark, so the paint gets lighter; on the
    // dark theme the text is light, so the paint gets darker.
    let (text, step) = match tone {
        Tone::Light => (text_rgb(Tone::Dark), 0.01),
        Tone::Dark => (text_rgb(Tone::Light), -0.01),
    };
    let mut rgb = hsl_to_rgb(hue.degrees(), s, l);
    while contrast_ratio(rgb, text) < 4.6 && (0.05..=0.97).contains(&(l + step)) {
        l += step;
        rgb = hsl_to_rgb(hue.degrees(), s, l);
    }
    rgb
}

/// Every palette cell in grid order: ten hues, each with its three shades.
pub fn palette_cells() -> Vec<HighlightColor> {
    Hue::ALL
        .iter()
        .flat_map(|&hue| Shade::ALL.iter().map(move |&shade| HighlightColor::palette(hue, shade)))
        .collect()
}

/// The color a new Highlight Rule gets: the palette cycled in hue order, at
/// the medium shade.
pub fn next_rule_color(rules_so_far: usize) -> HighlightColor {
    HighlightColor::palette(Hue::ALL[rules_so_far % Hue::ALL.len()], Shade::Medium)
}

/// Light text on a dark background, dark text on a light one, by WCAG
/// contrast.
pub fn text_tone_on(background: u32) -> Tone {
    let bg = relative_luminance(background);
    let on_white = (1.05) / (bg + 0.05);
    let on_black = (bg + 0.05) / 0.05;
    if on_black >= on_white { Tone::Dark } else { Tone::Light }
}

/// WCAG contrast ratio between two colors.
pub fn contrast_ratio(a: u32, b: u32) -> f32 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// Text color for `tone`: the near-white or near-black used on paint.
pub fn text_rgb(tone: Tone) -> u32 {
    match tone {
        Tone::Light => 0xF5F5F5,
        Tone::Dark => 0x141414,
    }
}

/// The colors used most recently, newest first, at most [`MAX_RECENT`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentColors {
    colors: Vec<HighlightColor>,
}

impl RecentColors {
    pub fn new() -> Self {
        Self::default()
    }

    /// Remember a color as the newest; an equal one moves to the front.
    pub fn push(&mut self, color: HighlightColor) {
        self.colors.retain(|c| *c != color);
        self.colors.insert(0, color);
        self.colors.truncate(MAX_RECENT);
    }

    pub fn colors(&self) -> &[HighlightColor] {
        &self.colors
    }
}

fn relative_luminance(rgb: u32) -> f32 {
    let channel = |shift: u32| {
        let c = ((rgb >> shift) & 0xFF) as f32 / 255.0;
        if c <= 0.03928 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
}

fn mix(base: u32, page: u32, keep: f32) -> u32 {
    let ch = |shift: u32| {
        let b = ((base >> shift) & 0xFF) as f32;
        let p = ((page >> shift) & 0xFF) as f32;
        (b * keep + p * (1.0 - keep)).round() as u32
    };
    (ch(16) << 16) | (ch(8) << 8) | ch(0)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> u32 {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = (h % 360.0) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r1, g1, b1) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let to8 = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u32;
    (to8(r1) << 16) | (to8(g1) << 8) | to8(b1)
}

/// `#RRGGBB` (or `RRGGBB`) as `0xRRGGBB`.
pub fn parse_hex(text: &str) -> Option<u32> {
    let t = text.trim().trim_start_matches('#');
    if t.len() != 6 || !t.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(t, 16).ok()
}

pub fn format_hex(rgb: u32) -> String {
    format!("#{:06X}", rgb & 0xFFFFFF)
}
