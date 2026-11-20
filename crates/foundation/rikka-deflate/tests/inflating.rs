use std::io::Write;

use flate2::Compression;
use flate2::write::{DeflateEncoder, GzEncoder, ZlibEncoder};
use rikka_deflate::{Deflater, Error, Inflater, Level};

use crate::support::{Random, prose, samples};

fn oracle_zlib(data: &[u8], level: u32) -> Vec<u8> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::new(level));
    encoder.write_all(data).expect("oracle deflates");
    encoder.finish().expect("oracle finishes")
}

#[test]
fn inflates_what_the_oracle_deflates() {
    for (name, data) in samples() {
        for level in [0, 1, 6, 9] {
            let mut encoder = DeflateEncoder::new(Vec::new(), Compression::new(level));
            encoder.write_all(&data).expect("oracle deflates");
            let raw = encoder.finish().expect("oracle finishes");
            let mut out = Vec::new();
            let consumed = Inflater::new()
                .inflate(&raw, &mut out, data.len())
                .unwrap_or_else(|error| panic!("{name} level {level}: {error}"));
            assert_eq!(consumed, raw.len(), "{name} level {level}");
            assert_eq!(out, data, "{name} level {level}");

            let mut out = Vec::new();
            Inflater::new()
                .zlib(&oracle_zlib(&data, level), &mut out, data.len())
                .unwrap_or_else(|error| panic!("{name} level {level}: {error}"));
            assert_eq!(out, data, "{name} level {level}");

            let mut encoder = GzEncoder::new(Vec::new(), Compression::new(level));
            encoder.write_all(&data).expect("oracle deflates");
            let gzip = encoder.finish().expect("oracle finishes");
            let mut out = Vec::new();
            Inflater::new()
                .gzip(&gzip, &mut out, data.len())
                .unwrap_or_else(|error| panic!("{name} level {level}: {error}"));
            assert_eq!(out, data, "{name} level {level}");
        }
    }
}

#[test]
fn output_limit_stops_a_bomb() {
    let bomb = oracle_zlib(&vec![0u8; 8 << 20], 9);
    assert!(bomb.len() < 16 << 10);
    let mut out = Vec::new();

    let error = Inflater::new()
        .zlib(&bomb, &mut out, 1 << 20)
        .expect_err("limit is a megabyte");

    assert_eq!(error, Error::TooLarge { limit: 1 << 20 });
    assert!(out.len() <= 1 << 20);
}

#[test]
fn inflating_appends_and_never_copies_from_earlier_output() {
    let stream = oracle_zlib(b"tail tail tail", 6);
    let mut out = b"head".to_vec();

    Inflater::new()
        .zlib(&stream, &mut out, 14)
        .expect("fits the limit");
    assert_eq!(out, b"headtail tail tail");

    let repeat_before_any_literal = [0x03, 0x02, 0x00];
    let mut out = b"aaaaaaaa".to_vec();
    assert_eq!(
        Inflater::new().inflate(&repeat_before_any_literal, &mut out, 64),
        Err(Error::Malformed)
    );
}

#[test]
fn damaged_streams_fail_cleanly() {
    let data = prose(20_000, 4);
    let stream = oracle_zlib(&data, 6);
    let mut out = Vec::new();

    for cut in [
        0,
        1,
        2,
        3,
        stream.len() / 2,
        stream.len() - 5,
        stream.len() - 1,
    ] {
        out.clear();
        let error = Inflater::new()
            .zlib(&stream[..cut], &mut out, data.len())
            .expect_err("truncated");
        assert_eq!(error, Error::Truncated, "cut at {cut}");
    }

    let mut wrong_sum = stream.clone();
    *wrong_sum.last_mut().expect("has a trailer") ^= 1;
    out.clear();
    assert_eq!(
        Inflater::new().zlib(&wrong_sum, &mut out, data.len()),
        Err(Error::Checksum)
    );

    out.clear();
    assert_eq!(
        Inflater::new().zlib(&[0x78, 0xBB, 0, 0], &mut out, 16),
        Err(Error::UnsupportedHeader)
    );
    assert_eq!(
        Inflater::new().inflate(&[0x07], &mut out, 16),
        Err(Error::Malformed)
    );

    let mut gzip = Vec::new();
    Deflater::new(Level::Fast).gzip(&data, &mut gzip);
    let size = gzip.len() - 1;
    gzip[size] ^= 0x40;
    out.clear();
    assert_eq!(
        Inflater::new().gzip(&gzip, &mut out, data.len()),
        Err(Error::Checksum)
    );
}

#[test]
fn corrupted_bytes_never_panic_or_exceed_the_limit() {
    let data = prose(6_000, 5);
    let stream = oracle_zlib(&data, 9);
    let mut random = Random::new(11);
    let mut inflater = Inflater::new();
    let mut out = Vec::new();

    for _ in 0..4_000 {
        let mut damaged = stream.clone();
        for _ in 0..1 + random.below(4) {
            let at = random.below(damaged.len());
            damaged[at] = random.next() as u8;
        }
        out.clear();
        let _ = inflater.zlib(&damaged, &mut out, data.len());
        assert!(out.len() <= data.len());
    }
}
