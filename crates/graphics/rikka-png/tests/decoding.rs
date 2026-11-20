use rikka_png::{Decoder, Layout, Limits};

use crate::support::{assemble, header, oracle_decode, texture, to_rgba8, zlib};

const ADAM7: [(usize, usize, usize, usize); 7] = [
    (0, 0, 8, 8),
    (4, 0, 8, 8),
    (0, 4, 4, 8),
    (2, 0, 4, 4),
    (0, 2, 2, 4),
    (1, 0, 2, 2),
    (0, 1, 1, 2),
];

struct Case {
    color: u8,
    depth: u8,
    channels: usize,
    keyed: bool,
}

fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for (color, channels, depths) in [
        (0u8, 1usize, &[1u8, 2, 4, 8, 16][..]),
        (2, 3, &[8, 16]),
        (3, 1, &[1, 2, 4, 8]),
        (4, 2, &[8, 16]),
        (6, 4, &[8, 16]),
    ] {
        for &depth in depths {
            for keyed in [false, true] {
                if keyed && matches!(color, 4 | 6) {
                    continue;
                }
                cases.push(Case {
                    color,
                    depth,
                    channels,
                    keyed,
                });
            }
        }
    }
    cases
}

fn filtered(scanline: &[u8], previous: &[u8], stride: usize, filter: u8) -> Vec<u8> {
    let at = |line: &[u8], index: usize| index.checked_sub(stride).map_or(0, |index| line[index]);
    let mut out = vec![filter];
    for (index, &value) in scanline.iter().enumerate() {
        let left = i16::from(at(scanline, index));
        let up = i16::from(previous[index]);
        let up_left = i16::from(at(previous, index));
        let predicted = match filter {
            0 => 0,
            1 => left,
            2 => up,
            3 => (left + up) / 2,
            _ => {
                let estimate = left + up - up_left;
                let distances = [
                    (estimate - left).abs(),
                    (estimate - up).abs(),
                    (estimate - up_left).abs(),
                ];
                if distances[0] <= distances[1] && distances[0] <= distances[2] {
                    left
                } else if distances[1] <= distances[2] {
                    up
                } else {
                    up_left
                }
            }
        };
        out.push(value.wrapping_sub(predicted as u8));
    }
    out
}

fn encode(case: &Case, width: usize, height: usize, interlaced: bool, seed: u64) -> Vec<u8> {
    let bits = case.channels * usize::from(case.depth);
    let stride = (bits / 8).max(1);
    let passes: &[(usize, usize, usize, usize)] = if interlaced { &ADAM7 } else { &[(0, 0, 1, 1)] };
    let mut raw = Vec::new();
    let mut filter = 0u8;
    for (pass, &(x0, y0, dx, dy)) in passes.iter().enumerate() {
        if x0 >= width || y0 >= height {
            continue;
        }
        let columns = (width - x0).div_ceil(dx);
        let rows = (height - y0).div_ceil(dy);
        let row_bytes = (columns * bits).div_ceil(8);
        let mut previous = vec![0u8; row_bytes];
        for row in 0..rows {
            let mut scanline = texture(row_bytes, seed + (pass * 1000 + row) as u64);
            if case.color == 3 && case.depth == 8 {
                for index in &mut scanline {
                    *index %= 16;
                }
            }
            raw.extend(filtered(&scanline, &previous, stride, filter % 5));
            filter += 1;
            previous = scanline;
        }
    }
    let mut chunks = vec![(
        b"IHDR",
        header(
            width as u32,
            height as u32,
            case.depth,
            case.color,
            u8::from(interlaced),
        ),
    )];
    if case.color == 3 {
        chunks.push((b"PLTE", texture(16 * 3, seed ^ 77)));
    }
    if case.keyed {
        chunks.push((
            b"tRNS",
            match case.color {
                0 => vec![0, 1],
                2 => vec![0, 3, 0, 4, 0, 5],
                _ => texture(9, seed ^ 99),
            },
        ));
    }
    chunks.push((b"IDAT", zlib(&raw)));
    chunks.push((b"IEND", Vec::new()));
    assemble(&chunks)
}

#[test]
fn every_color_type_depth_filter_and_interlace_matches_the_oracle() {
    let mut decoder = Decoder::new();
    let mut rgba = Vec::new();
    for case in cases() {
        for interlaced in [false, true] {
            for (width, height) in [(1, 1), (5, 3), (9, 17), (33, 8)] {
                let seed = u64::from(case.color) * 100 + u64::from(case.depth) + width as u64;
                let png = encode(&case, width, height, interlaced, seed);
                let label = format!(
                    "color {} depth {} keyed {} interlaced {interlaced} {width}x{height}",
                    case.color, case.depth, case.keyed
                );
                let expected = oracle_decode(&png);

                let decoded = decoder
                    .decode(&png, &Limits::UNLIMITED)
                    .unwrap_or_else(|error| panic!("{label}: {error}"));
                assert_eq!(decoded.picture, expected, "{label}");

                let size = decoder
                    .rgba8(&png, &Limits::UNLIMITED, &mut rgba)
                    .unwrap_or_else(|error| panic!("{label}: {error}"));
                assert_eq!(size, (width as u32, height as u32), "{label}");
                assert_eq!(rgba, to_rgba8(&expected), "{label}");
            }
        }
    }
}

#[test]
fn image_data_split_across_chunks_decodes_like_one_chunk() {
    let case = Case {
        color: 6,
        depth: 8,
        channels: 4,
        keyed: false,
    };
    let whole = encode(&case, 40, 30, false, 5);
    let parts = crate::support::chunks(&whole);
    let data = &parts[1].1;
    let mut chunks = vec![(b"IHDR", parts[0].1.clone())];
    for piece in data.chunks(7) {
        chunks.push((b"IDAT", piece.to_vec()));
    }
    chunks.push((b"IEND", Vec::new()));
    let split = assemble(&chunks);

    let mut decoder = Decoder::new();
    let one = decoder.decode(&whole, &Limits::UNLIMITED).expect("decodes");
    let many = decoder.decode(&split, &Limits::UNLIMITED).expect("decodes");

    assert_eq!(one, many);
    assert_eq!(one.picture.layout, Layout::Rgba8);
}
