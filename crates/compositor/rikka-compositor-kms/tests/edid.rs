use rikka_colorimetry::Primaries;
use rikka_compositor_kms::{Edid, HdrCapabilities};

const DESCRIPTORS: usize = 54;
const DESCRIPTOR_LEN: usize = 18;

fn edid(name: Option<&str>, serial: Option<&str>) -> Vec<u8> {
    let mut data = vec![0u8; 128];
    data[0..8].copy_from_slice(&[0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0]);
    let packed: u16 = (4 << 10) | (5 << 5) | 12;
    data[8..10].copy_from_slice(&packed.to_be_bytes());
    data[10..12].copy_from_slice(&0x40a1u16.to_le_bytes());
    let mut slot = 0;
    for (tag, text) in [(0xfc, name), (0xff, serial)] {
        let Some(text) = text else {
            continue;
        };
        let start = DESCRIPTORS + slot * DESCRIPTOR_LEN;
        data[start + 3] = tag;
        let body = &mut data[start + 5..start + DESCRIPTOR_LEN];
        body.fill(b' ');
        body[..text.len()].copy_from_slice(text.as_bytes());
        if text.len() < body.len() {
            body[text.len()] = 0x0a;
        }
        slot += 1;
    }
    data
}

#[test]
fn edid_names_monitor_or_product_code() {
    let parsed = Edid::parse(&edid(Some("DELL U2720Q"), Some("ABC123"))).expect("valid");
    assert_eq!(parsed.make, "DEL");
    assert_eq!(parsed.model, "DELL U2720Q");
    assert_eq!(parsed.serial.as_deref(), Some("ABC123"));

    let unnamed = Edid::parse(&edid(None, None)).expect("valid");
    assert_eq!(unnamed.model, "0x40A1");
    assert_eq!(unnamed.serial, None);
}

#[test]
fn truncated_edid_yields_nothing() {
    assert_eq!(Edid::parse(&[0; 64]), None);
}

fn chromaticity(data: &mut [u8], primaries: &Primaries) {
    let points = [
        primaries.red,
        primaries.green,
        primaries.blue,
        primaries.white,
    ];
    let mut low: u16 = 0;
    for (index, value) in points
        .iter()
        .flat_map(|point| [point.x, point.y])
        .enumerate()
    {
        let code = (value * 1024.0).round() as u16;
        data[27 + index] = (code >> 2) as u8;
        low |= (code & 0b11) << (14 - 2 * index);
    }
    data[25..27].copy_from_slice(&low.to_be_bytes());
}

fn with_hdr_extension(mut data: Vec<u8>, eotfs: u8, luminance: [u8; 3]) -> Vec<u8> {
    data[126] = 1;
    let mut block = vec![0u8; 128];
    block[0] = 0x02;
    block[1] = 3;
    let colorimetry = [0xe0 | 2, 0x05, 0x80];
    let hdr = [
        0xe0 | 6,
        0x06,
        eotfs,
        0x01,
        luminance[0],
        luminance[1],
        luminance[2],
    ];
    let mut at = 4;
    for data_block in [&colorimetry[..], &hdr[..]] {
        block[at..at + data_block.len()].copy_from_slice(data_block);
        at += data_block.len();
    }
    block[2] = at as u8;
    data.extend(block);
    data
}

#[test]
fn edid_reports_chromaticity_and_hdr_static_metadata() {
    let mut base = edid(Some("HDR"), None);
    chromaticity(&mut base, &Primaries::DISPLAY_P3);
    let parsed = Edid::parse(&with_hdr_extension(base, 0b0000_0101, [96, 80, 64])).expect("valid");
    let primaries = parsed.primaries.expect("chromaticity is set");
    for (got, want) in [
        (primaries.red.x, 0.680),
        (primaries.green.y, 0.690),
        (primaries.blue.x, 0.150),
        (primaries.white.y, 0.329),
    ] {
        assert!((got - want).abs() < 1.0e-3, "{got} is not {want}");
    }
    let hdr: HdrCapabilities = parsed.hdr.expect("the CTA block announces PQ");
    assert!(hdr.pq && hdr.traditional_sdr && !hdr.hlg && hdr.bt2020_rgb);
    let max = hdr.max_luminance.unwrap();
    assert!(
        (max - 400.0).abs() < 1.0e-9,
        "CV 96 is 50 * 2^3 nits, not {max}"
    );
    assert!((hdr.max_frame_average.unwrap() - 50.0 * 2f64.powf(2.5)).abs() < 1.0e-9);
    let min = hdr.min_luminance.unwrap();
    let expected = 400.0 * (64.0f64 / 255.0).powi(2) / 100.0;
    assert!((min - expected).abs() < 1.0e-9, "{min}");
}

#[test]
fn edid_without_an_hdr_eotf_has_no_hdr_capabilities() {
    let parsed = Edid::parse(&with_hdr_extension(edid(None, None), 0b0000_0001, [0; 3])).unwrap();
    assert_eq!(parsed.hdr, None);
    assert_eq!(parsed.primaries, None);
}
