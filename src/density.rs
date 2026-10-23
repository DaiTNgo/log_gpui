use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Density {
    Comfortable,
    #[default]
    Compact,
    UltraCompact,
}

impl Density {
    pub fn row_height(&self) -> f32 {
        match self {
            Density::Comfortable => 30.0,
            Density::Compact => 24.0,
            Density::UltraCompact => 20.0,
        }
    }

    pub fn font_size(&self) -> f32 {
        match self {
            Density::Comfortable => 13.0,
            Density::Compact => 12.0,
            Density::UltraCompact => 11.0,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Density::Comfortable => "Comfortable",
            Density::Compact => "Compact",
            Density::UltraCompact => "Ultra Compact",
        }
    }
}

/// Steps the text can be made smaller or larger by, from the Density's size.
pub const ZOOM_MIN: i8 = -3;
pub const ZOOM_MAX: i8 = 8;

/// The size the log text is drawn at: the Density's, made larger or smaller
/// by the zoom steps (`⌘+` / `⌘−`), one point per step.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TextScale {
    pub density: Density,
    pub zoom: i8,
}

impl From<Density> for TextScale {
    fn from(density: Density) -> Self {
        Self { density, zoom: 0 }
    }
}

impl TextScale {
    pub fn new(density: Density, zoom: i8) -> Self {
        Self { density, zoom: zoom.clamp(ZOOM_MIN, ZOOM_MAX) }
    }

    pub fn font_size(&self) -> f32 {
        self.density.font_size() + f32::from(self.zoom)
    }

    /// The row grows with the text, keeping the Density's spacing.
    pub fn row_height(&self) -> f32 {
        (self.density.row_height() * self.ratio()).round()
    }

    /// How much larger than the Density's own size the text is.
    pub fn ratio(&self) -> f32 {
        self.font_size() / self.density.font_size()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_steps_the_text_and_rows_within_bounds() {
        let base = TextScale::new(Density::Compact, 0);
        assert_eq!((base.font_size(), base.row_height()), (12.0, 24.0));
        let larger = TextScale::new(Density::Compact, 3);
        assert_eq!((larger.font_size(), larger.row_height()), (15.0, 30.0));
        assert_eq!(TextScale::new(Density::Compact, 100).zoom, ZOOM_MAX);
        assert_eq!(TextScale::new(Density::Compact, -100).zoom, ZOOM_MIN);
    }
}
