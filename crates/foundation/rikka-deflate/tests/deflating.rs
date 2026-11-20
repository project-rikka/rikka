use std::io::Read;

use flate2::Compression;
use flate2::read::{DeflateDecoder, GzDecoder, ZlibDecoder};
use flate2::write::DeflateEncoder;
use rikka_deflate::{Deflater, Inflater, Level};

use crate::support::{prose, samples};

const LEVELS: [Level; 3] = [Level::Fast, Level::Balanced, Level::Best];

fn read_all(mut reader: impl Read) -> Vec<u8> {
    let mut out = Vec::new();
    reader.read_to_end(&mut out).expect("oracle inflates");
    out
}

#[test]
fn every_level_round_trips_through_the_oracle_and_the_inflater() {
    for (name, data) in samples() {
        for level in LEVELS {
            let mut deflater = Deflater::new(level);

            let mut raw = Vec::new();
            deflater.deflate(&data, &mut raw);
            assert_eq!(
                read_all(DeflateDecoder::new(&raw[..])),
                data,
                "{name} {level:?}"
            );
            let mut restored = Vec::new();
            let consumed = Inflater::new()
                .inflate(&raw, &mut restored, data.len())
                .expect("inflates");
            assert_eq!(consumed, raw.len(), "{name} {level:?}");
            assert_eq!(restored, data, "{name} {level:?}");

            let mut zlib = Vec::new();
            deflater.zlib(&data, &mut zlib);
            assert_eq!(
                read_all(ZlibDecoder::new(&zlib[..])),
                data,
                "{name} {level:?}"
            );

            let mut gzip = Vec::new();
            deflater.gzip(&data, &mut gzip);
            assert_eq!(
                read_all(GzDecoder::new(&gzip[..])),
                data,
                "{name} {level:?}"
            );
        }
    }
}

#[test]
fn output_is_appended_after_existing_bytes() {
    let mut out = b"head".to_vec();
    Deflater::new(Level::Balanced).zlib(b"body body body body", &mut out);

    assert_eq!(&out[..4], b"head");
    assert_eq!(
        read_all(ZlibDecoder::new(&out[4..])),
        b"body body body body"
    );
}

#[test]
fn compresses_about_as_well_as_the_oracle() {
    let data = prose(400_000, 9);
    let oracle = |level: u32| {
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::new(level));
        std::io::Write::write_all(&mut encoder, &data).expect("oracle deflates");
        encoder.finish().expect("oracle finishes").len()
    };
    let ours = |level: Level| {
        let mut out = Vec::new();
        Deflater::new(level).deflate(&data, &mut out);
        out.len()
    };

    let (fast, balanced, best) = (ours(Level::Fast), ours(Level::Balanced), ours(Level::Best));

    assert!(
        best <= balanced && balanced <= fast,
        "{best} {balanced} {fast}"
    );
    assert!(
        balanced * 100 <= oracle(6) * 108,
        "{balanced} vs {}",
        oracle(6)
    );
    assert!(best * 100 <= oracle(9) * 108, "{best} vs {}", oracle(9));
    assert!(fast * 100 <= oracle(1) * 125, "{fast} vs {}", oracle(1));
}
