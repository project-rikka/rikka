use std::io::Cursor;

use rikka_png::{Decoder, Encoder, Error, Layout, Level, Limits, Metadata, Pixels, Samples, Text};

use crate::support::{oracle_decode, texture};

const LAYOUTS: [Layout; 8] = [
    Layout::L8,
    Layout::La8,
    Layout::Rgb8,
    Layout::Rgba8,
    Layout::L16,
    Layout::La16,
    Layout::Rgb16,
    Layout::Rgba16,
];

fn samples(layout: Layout, bytes: &[u8]) -> Samples {
    if layout.is_deep() {
        Samples::U16(
            bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u16::from_ne_bytes(*pair))
                .collect(),
        )
    } else {
        Samples::U8(bytes.to_vec())
    }
}

#[test]
fn every_layout_and_level_decodes_back_in_the_oracle_and_the_decoder() {
    let mut decoder = Decoder::new();
    for layout in LAYOUTS {
        for level in [Level::Fast, Level::Balanced, Level::Best] {
            for (width, height) in [(1u32, 1u32), (7, 5), (64, 33)] {
                let bytes = texture(
                    width as usize * height as usize * layout.bytes_per_pixel(),
                    u64::from(width) + layout.channels() as u64,
                );
                let mut png = Vec::new();
                Encoder::new(level)
                    .encode(
                        Pixels {
                            width,
                            height,
                            layout,
                            bytes: &bytes,
                        },
                        &Metadata::default(),
                        &mut png,
                    )
                    .expect("encodes");
                let label = format!("{layout:?} {level:?} {width}x{height}");

                let oracle = oracle_decode(&png);
                assert_eq!(oracle.layout, layout, "{label}");
                assert_eq!((oracle.width, oracle.height), (width, height), "{label}");
                assert_eq!(oracle.samples, samples(layout, &bytes), "{label}");

                let decoded = decoder.decode(&png, &Limits::UNLIMITED).expect("decodes");
                assert_eq!(decoded.picture, oracle, "{label}");
            }
        }
    }
}

#[test]
fn profile_and_text_reach_the_oracle() {
    let profile = texture(3_000, 8);
    let text = [
        Text {
            keyword: "Thumb::URI".into(),
            text: "file:///home/person/caf\u{e9}.png".into(),
        },
        Text {
            keyword: "Title".into(),
            text: "\u{5199}\u{771f}".into(),
        },
        Text {
            keyword: String::new(),
            text: "dropped".into(),
        },
    ];
    let mut png = Vec::new();
    Encoder::new(Level::Balanced)
        .encode(
            Pixels {
                width: 2,
                height: 2,
                layout: Layout::Rgb8,
                bytes: &[9; 12],
            },
            &Metadata {
                icc_profile: Some(&profile),
                text: &text,
            },
            &mut png,
        )
        .expect("encodes");

    let reader = png::Decoder::new(Cursor::new(&png))
        .read_info()
        .expect("oracle reads");
    let info = reader.info();
    assert_eq!(info.icc_profile.as_deref(), Some(&profile[..]));
    assert_eq!(info.uncompressed_latin1_text.len(), 1);
    assert_eq!(info.uncompressed_latin1_text[0].keyword, "Thumb::URI");
    assert_eq!(info.uncompressed_latin1_text[0].text, text[0].text);
    assert_eq!(info.utf8_text.len(), 1);
    assert_eq!(info.utf8_text[0].get_text().expect("plain"), text[1].text);

    let decoded = Decoder::new()
        .decode(&png, &Limits::UNLIMITED)
        .expect("decodes");
    assert_eq!(decoded.icc_profile.as_deref(), Some(&profile[..]));
}

#[test]
fn refuses_sizes_that_do_not_describe_the_pixels() {
    let mut out = Vec::new();
    let mut encoder = Encoder::new(Level::Fast);
    let pixels = |width, height, bytes| Pixels {
        width,
        height,
        layout: Layout::Rgba8,
        bytes,
    };

    assert!(matches!(
        encoder.encode(pixels(0, 4, &[]), &Metadata::default(), &mut out),
        Err(Error::InvalidSize { .. })
    ));
    assert!(matches!(
        encoder.encode(pixels(2, 2, &[0; 15]), &Metadata::default(), &mut out),
        Err(Error::PixelDataSize {
            len: 15,
            expected: 16
        })
    ));
}
