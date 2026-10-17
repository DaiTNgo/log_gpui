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
