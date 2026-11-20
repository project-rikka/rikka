use rikka_png::{Decoder, Error, Limits};

use crate::support::{Random, assemble, chunks, header, rgba_png, texture, zlib};

fn sample() -> Vec<u8> {
    rgba_png(12, 9, &texture(12 * 9 * 4, 3))
}

fn decode(png: &[u8]) -> Result<rikka_png::Decoded, Error> {
    Decoder::new().decode(png, &Limits::UNLIMITED)
}

#[test]
fn every_truncation_fails_without_panicking() {
    let png = sample();

    for cut in 0..png.len() - 12 {
        let error = decode(&png[..cut]).expect_err("truncated");
        assert!(
            matches!(
                error,
                Error::Signature | Error::Truncated | Error::NoImageData | Error::Data(_)
            ),
            "cut at {cut}: {error:?}"
        );
    }
    assert!(
        decode(&png[..png.len() - 12]).is_ok(),
        "only IEND is missing"
    );
}

#[test]
fn checksums_guard_critical_chunks_and_skip_ancillary_ones() {
    let mut png = sample();
    let last_data_byte = png.len() - 12 - 4 - 1;
    png[last_data_byte] ^= 0x10;
    assert!(matches!(decode(&png), Err(Error::Checksum(kind)) if &kind.0 == b"IDAT"));

    let parts = chunks(&sample());
    let mut with_text = assemble(&[
        (b"IHDR", parts[0].1.clone()),
        (b"tEXt", b"Title\0picture".to_vec()),
        (b"IDAT", parts[1].1.clone()),
        (b"IEND", Vec::new()),
    ]);
    let text_at = 8 + 12 + 13 + 8;
    with_text[text_at] ^= 0x01;
    assert_eq!(
        decode(&with_text).expect("ancillary damage is skipped"),
        decode(&sample()).expect("decodes")
    );
}

#[test]
fn limits_refuse_oversized_headers_before_allocating() {
    let huge = assemble(&[
        (b"IHDR", header(0x7FFF_FFFF, 0x7FFF_FFFF, 16, 6, 0)),
        (b"IDAT", zlib(&[0; 64])),
        (b"IEND", Vec::new()),
    ]);
    assert!(matches!(
        decode(&huge),
        Err(Error::LimitsExceeded {
            width: 0x7FFF_FFFF,
            height: 0x7FFF_FFFF
        })
    ));

    let png = sample();
    let mut decoder = Decoder::new();
    let limits = |max_width, max_height, max_bytes| Limits {
        max_width,
        max_height,
        max_bytes,
    };
    for refused in [
        limits(11, 100, u64::MAX),
        limits(100, 8, u64::MAX),
        limits(100, 100, 12 * 9 * 4 - 1),
    ] {
        assert!(matches!(
            decoder.decode(&png, &refused),
            Err(Error::LimitsExceeded { .. })
        ));
        let mut rgba = Vec::new();
        assert!(matches!(
            decoder.rgba8(&png, &refused, &mut rgba),
            Err(Error::LimitsExceeded { .. })
        ));
    }
    assert!(decoder.decode(&png, &limits(12, 9, 12 * 9 * 4)).is_ok());
}

#[test]
fn image_data_larger_or_smaller_than_the_header_is_refused() {
    let bomb = assemble(&[
        (b"IHDR", header(4, 4, 8, 6, 0)),
        (b"IDAT", zlib(&vec![0; 64 << 20])),
        (b"IEND", Vec::new()),
    ]);
    assert!(matches!(
        decode(&bomb),
        Err(Error::Data(rikka_deflate::Error::TooLarge { limit: 68 }))
    ));

    let short = assemble(&[
        (b"IHDR", header(4, 4, 8, 6, 0)),
        (b"IDAT", zlib(&[0; 67])),
        (b"IEND", Vec::new()),
    ]);
    assert!(matches!(decode(&short), Err(Error::Data(_))));
}

#[test]
fn structural_damage_is_reported() {
    let parts = chunks(&sample());
    let ihdr = parts[0].1.clone();
    let idat = parts[1].1.clone();
    let mut raw = vec![0u8; (12 * 4 + 1) * 9];
    raw[0] = 5;

    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("not png", b"GIF89a not a png at all".to_vec()),
        (
            "no header",
            assemble(&[(b"IDAT", idat.clone()), (b"IEND", Vec::new())]),
        ),
        (
            "two headers",
            assemble(&[(b"IHDR", ihdr.clone()), (b"IHDR", ihdr.clone())]),
        ),
        (
            "no data",
            assemble(&[(b"IHDR", ihdr.clone()), (b"IEND", Vec::new())]),
        ),
        (
            "unknown critical chunk",
            assemble(&[
                (b"IHDR", ihdr.clone()),
                (b"ABCD", vec![1, 2, 3]),
                (b"IDAT", idat.clone()),
            ]),
        ),
        (
            "unknown filter",
            assemble(&[(b"IHDR", ihdr.clone()), (b"IDAT", zlib(&raw))]),
        ),
        (
            "bad depth",
            assemble(&[(b"IHDR", header(4, 4, 3, 6, 0)), (b"IDAT", idat.clone())]),
        ),
        (
            "zero width",
            assemble(&[(b"IHDR", header(0, 4, 8, 6, 0)), (b"IDAT", idat.clone())]),
        ),
        (
            "palette missing",
            assemble(&[(b"IHDR", header(4, 4, 8, 3, 0)), (b"IDAT", zlib(&[0; 20]))]),
        ),
        (
            "palette index out of range",
            assemble(&[
                (b"IHDR", header(4, 4, 8, 3, 0)),
                (b"PLTE", vec![1, 2, 3]),
                (b"IDAT", zlib(&[0, 0, 0, 1, 0].repeat(4))),
            ]),
        ),
        (
            "data after other chunks",
            assemble(&[
                (b"IHDR", ihdr.clone()),
                (b"IDAT", idat[..10].to_vec()),
                (b"tEXt", b"a\0b".to_vec()),
                (b"IDAT", idat[10..].to_vec()),
            ]),
        ),
    ];

    for (name, png) in cases {
        assert!(decode(&png).is_err(), "{name} decoded");
    }
}

#[test]
fn damaged_chunk_bodies_never_panic() {
    let parts = chunks(&sample());
    let mut random = Random::new(21);
    let mut decoder = Decoder::new();
    let mut rgba = Vec::new();
    let limits = Limits {
        max_width: 64,
        max_height: 64,
        max_bytes: 1 << 16,
    };

    for _ in 0..3_000 {
        let mut damaged: Vec<([u8; 4], Vec<u8>)> = parts.clone();
        for _ in 0..1 + random.below(3) {
            let chunk = random.below(2);
            let body = &mut damaged[chunk].1;
            let at = random.below(body.len());
            body[at] = random.next() as u8;
        }
        let rebuilt: Vec<(&[u8; 4], Vec<u8>)> = damaged
            .iter()
            .map(|(kind, body)| (kind, body.clone()))
            .collect();
        let png = assemble(&rebuilt);
        let _ = decoder.decode(&png, &limits);
        let _ = decoder.rgba8(&png, &limits, &mut rgba);
        let _ = decoder.animation(&png, &limits);
        let _ = rikka_png::probe(std::io::Cursor::new(&png));
    }
}
