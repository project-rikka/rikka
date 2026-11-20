use rikka_colorimetry::{Chromaticity, Primaries};

const BASE_BLOCK: usize = 128;
const BLOCK: usize = 128;
const DESCRIPTORS: usize = 54;
const DESCRIPTOR_LEN: usize = 18;
const TAG_MONITOR_NAME: u8 = 0xfc;
const TAG_SERIAL: u8 = 0xff;
const UNKNOWN: &str = "Unknown";

const CHROMATICITY_LOW: usize = 25;
const CHROMATICITY_HIGH: usize = 27;
const EXTENSION_COUNT: usize = 126;
const CTA_EXTENSION: u8 = 0x02;
const CTA_DATA_BLOCKS: usize = 4;
const TAG_EXTENDED: u8 = 7;
const EXTENDED_COLORIMETRY: u8 = 5;
const EXTENDED_HDR_STATIC_METADATA: u8 = 6;
const COLORIMETRY_BT2020_RGB: u8 = 1 << 7;
const EOTF_TRADITIONAL_SDR: u8 = 1 << 0;
const EOTF_PQ: u8 = 1 << 2;
const EOTF_HLG: u8 = 1 << 3;
const LUMINANCE_BASE: f64 = 50.0;
const LUMINANCE_STEPS: f64 = 32.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Edid {
    pub make: String,
    pub model: String,
    pub serial: Option<String>,
    pub primaries: Option<Primaries>,
    pub hdr: Option<HdrCapabilities>,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct HdrCapabilities {
    pub traditional_sdr: bool,
    pub pq: bool,
    pub hlg: bool,
    pub bt2020_rgb: bool,
    pub max_luminance: Option<f64>,
    pub max_frame_average: Option<f64>,
    pub min_luminance: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub make: String,
    pub model: String,
    pub serial: Option<String>,
    pub connector: String,
}

impl Edid {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < BASE_BLOCK {
            return None;
        }
        Some(Self {
            make: manufacturer(data),
            model: text_descriptor(data, TAG_MONITOR_NAME)
                .unwrap_or_else(|| format!("0x{:04X}", product_code(data))),
            serial: text_descriptor(data, TAG_SERIAL),
            primaries: primaries(data),
            hdr: hdr_capabilities(data),
        })
    }
}

impl Identity {
    pub(crate) fn new(edid: Option<&Edid>, connector: &str) -> Self {
        match edid {
            Some(edid) => Self {
                make: edid.make.clone(),
                model: edid.model.clone(),
                serial: edid.serial.clone(),
                connector: connector.to_owned(),
            },
            None => Self {
                make: UNKNOWN.to_owned(),
                model: UNKNOWN.to_owned(),
                serial: None,
                connector: connector.to_owned(),
            },
        }
    }

    pub fn key(&self) -> String {
        match &self.serial {
            Some(serial) => format!("{} {} {serial}", self.make, self.model),
            None => self.connector.clone(),
        }
    }
}

fn manufacturer(data: &[u8]) -> String {
    let packed = u16::from_be_bytes([data[8], data[9]]);
    (0..3)
        .rev()
        .map(|index| {
            let value = ((packed >> (index * 5)) & 0x1f) as u8;
            match value {
                1..=26 => char::from(b'A' + value - 1),
                _ => '?',
            }
        })
        .collect()
}

fn product_code(data: &[u8]) -> u16 {
    u16::from_le_bytes([data[10], data[11]])
}

fn text_descriptor(data: &[u8], tag: u8) -> Option<String> {
    (0..4).find_map(|index| {
        let start = DESCRIPTORS + index * DESCRIPTOR_LEN;
        let descriptor = &data[start..start + DESCRIPTOR_LEN];
        if descriptor[0..3] != [0, 0, 0] || descriptor[3] != tag {
            return None;
        }
        let text: String = descriptor[5..]
            .iter()
            .take_while(|byte| **byte != 0x0a)
            .map(|byte| char::from(*byte))
            .collect();
        let text = text.trim();
        (!text.is_empty()).then(|| text.to_owned())
    })
}

fn primaries(data: &[u8]) -> Option<Primaries> {
    let low = u16::from(data[CHROMATICITY_LOW]) << 8 | u16::from(data[CHROMATICITY_LOW + 1]);
    let coordinate = |index: usize| {
        let high = u16::from(data[CHROMATICITY_HIGH + index]);
        let bits = (low >> (14 - 2 * index)) & 0b11;
        f64::from(high << 2 | bits) / 1024.0
    };
    let point = |index: usize| Chromaticity::new(coordinate(index), coordinate(index + 1));
    let primaries = Primaries::new(point(0), point(2), point(4), point(6));
    primaries.is_valid().then_some(primaries)
}

fn hdr_capabilities(data: &[u8]) -> Option<HdrCapabilities> {
    let extensions = usize::from(data[EXTENSION_COUNT]);
    let mut found: Option<HdrCapabilities> = None;
    for index in 1..=extensions {
        let Some(block) = data.get(index * BLOCK..(index + 1) * BLOCK) else {
            break;
        };
        if block[0] != CTA_EXTENSION {
            continue;
        }
        let end = usize::from(block[2]).clamp(CTA_DATA_BLOCKS, BLOCK);
        let mut at = CTA_DATA_BLOCKS;
        while at < end {
            let header = block[at];
            let length = usize::from(header & 0x1f);
            let Some(payload) = block.get(at + 1..at + 1 + length) else {
                break;
            };
            if header >> 5 == TAG_EXTENDED
                && let Some((&extended, body)) = payload.split_first()
            {
                let capabilities = found.get_or_insert_default();
                match extended {
                    EXTENDED_COLORIMETRY => {
                        capabilities.bt2020_rgb = body
                            .first()
                            .is_some_and(|bits| bits & COLORIMETRY_BT2020_RGB != 0);
                    }
                    EXTENDED_HDR_STATIC_METADATA => static_metadata(body, capabilities),
                    _ => {}
                }
            }
            at += 1 + length;
        }
    }
    found.filter(|capabilities| capabilities.pq || capabilities.hlg)
}

fn static_metadata(body: &[u8], capabilities: &mut HdrCapabilities) {
    let Some(&eotfs) = body.first() else {
        return;
    };
    capabilities.traditional_sdr = eotfs & EOTF_TRADITIONAL_SDR != 0;
    capabilities.pq = eotfs & EOTF_PQ != 0;
    capabilities.hlg = eotfs & EOTF_HLG != 0;
    let code = |at: usize| body.get(at).copied().filter(|code| *code != 0);
    let luminance = |code: u8| LUMINANCE_BASE * 2f64.powf(f64::from(code) / LUMINANCE_STEPS);
    capabilities.max_luminance = code(2).map(luminance);
    capabilities.max_frame_average = code(3).map(luminance);
    capabilities.min_luminance = code(4).zip(capabilities.max_luminance).map(|(code, max)| {
        let fraction = f64::from(code) / 255.0;
        max * fraction * fraction / 100.0
    });
}
