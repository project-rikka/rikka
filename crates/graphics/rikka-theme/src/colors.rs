use rikka_types::Color;
use serde::{Deserialize, Deserializer};

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ThemeColors {
    #[serde(deserialize_with = "hex_color")]
    pub crust: Color,
    #[serde(deserialize_with = "hex_color")]
    pub mantle: Color,
    #[serde(deserialize_with = "hex_color")]
    pub base: Color,
    #[serde(deserialize_with = "hex_color")]
    pub surface: Color,
    #[serde(deserialize_with = "hex_color")]
    pub overlay: Color,
    #[serde(deserialize_with = "hex_color")]
    pub text: Color,
    #[serde(deserialize_with = "hex_color")]
    pub subtext: Color,
    #[serde(deserialize_with = "hex_color")]
    pub muted: Color,
    #[serde(deserialize_with = "hex_color")]
    pub border: Color,
    #[serde(deserialize_with = "hex_color")]
    pub cursor: Color,
    #[serde(deserialize_with = "hex_color")]
    pub highlight: Color,
    #[serde(deserialize_with = "hex_color")]
    pub link: Color,
    #[serde(deserialize_with = "hex_color")]
    pub accent: Color,
    #[serde(deserialize_with = "hex_color")]
    pub red: Color,
    #[serde(deserialize_with = "hex_color")]
    pub orange: Color,
    #[serde(deserialize_with = "hex_color")]
    pub yellow: Color,
    #[serde(deserialize_with = "hex_color")]
    pub green: Color,
    #[serde(deserialize_with = "hex_color")]
    pub cyan: Color,
    #[serde(deserialize_with = "hex_color")]
    pub blue: Color,
    #[serde(deserialize_with = "hex_color")]
    pub purple: Color,
    #[serde(deserialize_with = "hex_color")]
    pub magenta: Color,
    #[serde(deserialize_with = "hex_color")]
    pub error: Color,
    #[serde(deserialize_with = "hex_color")]
    pub warning: Color,
    #[serde(deserialize_with = "hex_color")]
    pub success: Color,
    #[serde(deserialize_with = "hex_color")]
    pub info: Color,
}

fn hex_color<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Color, D::Error> {
    let value = String::deserialize(deserializer)?;
    Color::parse_hex(&value).ok_or_else(|| {
        serde::de::Error::custom(format_args!(
            "invalid hex color `{value}`: expected #rgb, #rgba, #rrggbb, or #rrggbbaa"
        ))
    })
}
