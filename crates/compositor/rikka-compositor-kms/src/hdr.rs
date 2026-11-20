use rikka_colorimetry::Primaries;

pub const HDR_OUTPUT_METADATA_SIZE: usize = 32;

const STATIC_METADATA_TYPE1: u8 = 0;
const EOTF_SMPTE_ST2084: u8 = 2;
const CHROMATICITY_UNIT: f64 = 50_000.0;
const MIN_LUMINANCE_UNIT: f64 = 10_000.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HdrMetadata {
    pub primaries: Primaries,
    pub max_luminance: f64,
    pub min_luminance: f64,
    pub max_cll: f64,
    pub max_fall: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Signal {
    #[default]
    Sdr,
    Pq(HdrMetadata),
}

impl HdrMetadata {
    pub fn blob(&self) -> [u8; HDR_OUTPUT_METADATA_SIZE] {
        let mut bytes = [0u8; HDR_OUTPUT_METADATA_SIZE];
        bytes[0..4].copy_from_slice(&u32::from(STATIC_METADATA_TYPE1).to_ne_bytes());
        bytes[4] = EOTF_SMPTE_ST2084;
        bytes[5] = STATIC_METADATA_TYPE1;
        let primaries = &self.primaries;
        let points = [
            primaries.red,
            primaries.green,
            primaries.blue,
            primaries.white,
        ];
        for (index, point) in points.iter().enumerate() {
            let at = 6 + index * 4;
            bytes[at..at + 2].copy_from_slice(&chromaticity(point.x).to_ne_bytes());
            bytes[at + 2..at + 4].copy_from_slice(&chromaticity(point.y).to_ne_bytes());
        }
        let levels = [
            nits(self.max_luminance),
            scaled(self.min_luminance * MIN_LUMINANCE_UNIT),
            nits(self.max_cll),
            nits(self.max_fall),
        ];
        for (index, level) in levels.iter().enumerate() {
            let at = 22 + index * 2;
            bytes[at..at + 2].copy_from_slice(&level.to_ne_bytes());
        }
        bytes
    }
}

fn chromaticity(coordinate: f64) -> u16 {
    scaled(coordinate * CHROMATICITY_UNIT)
}

fn nits(luminance: f64) -> u16 {
    scaled(luminance)
}

fn scaled(value: f64) -> u16 {
    value.round().clamp(0.0, f64::from(u16::MAX)) as u16
}
