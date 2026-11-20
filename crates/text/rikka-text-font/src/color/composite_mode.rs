#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CompositeMode {
    Clear,
    Source,
    Destination,
    #[default]
    SourceOver,
    DestinationOver,
    SourceIn,
    DestinationIn,
    SourceOut,
    DestinationOut,
    SourceAtop,
    DestinationAtop,
    Xor,
    Plus,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
    Multiply,
    HslHue,
    HslSaturation,
    HslColor,
    HslLuminosity,
}

impl CompositeMode {
    const ALL: [Self; 28] = [
        Self::Clear,
        Self::Source,
        Self::Destination,
        Self::SourceOver,
        Self::DestinationOver,
        Self::SourceIn,
        Self::DestinationIn,
        Self::SourceOut,
        Self::DestinationOut,
        Self::SourceAtop,
        Self::DestinationAtop,
        Self::Xor,
        Self::Plus,
        Self::Screen,
        Self::Overlay,
        Self::Darken,
        Self::Lighten,
        Self::ColorDodge,
        Self::ColorBurn,
        Self::HardLight,
        Self::SoftLight,
        Self::Difference,
        Self::Exclusion,
        Self::Multiply,
        Self::HslHue,
        Self::HslSaturation,
        Self::HslColor,
        Self::HslLuminosity,
    ];

    pub(crate) fn from_raw(raw: u8) -> Self {
        Self::ALL.get(usize::from(raw)).copied().unwrap_or_default()
    }
}
