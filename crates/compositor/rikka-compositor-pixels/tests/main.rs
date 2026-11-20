use rikka_compositor_geometry::{Buffer, Rect, Size};
use rikka_compositor_pixels::{
    SHM_FORMATS, area_to_rgba, damaged_rects, write_deep_rows, write_rows,
};
use rikka_compositor_types::Fourcc;

fn rect(x: i32, y: i32, w: i32, h: i32) -> Rect<i32, Buffer> {
    Rect::new((x, y).into(), (w, h).into())
}

fn one(format: Fourcc, texel: &[u8]) -> Vec<u8> {
    area_to_rgba(texel, format, texel.len(), rect(0, 0, 1, 1)).expect("one whole texel")
}

fn ten(red: u32, green: u32, blue: u32, alpha: u32) -> [u8; 4] {
    (red | green << 10 | blue << 20 | alpha << 30).to_le_bytes()
}

fn halves(channels: [u16; 4]) -> Vec<u8> {
    channels.into_iter().flat_map(u16::to_le_bytes).collect()
}

#[test]
fn every_advertised_format_converts() {
    for format in SHM_FORMATS {
        let texel = [0u8; 8];
        let bytes = match format {
            Fourcc::ABGR16161616F | Fourcc::XBGR16161616F => 8,
            Fourcc::RGB565 => 2,
            _ => 4,
        };
        assert!(
            area_to_rgba(&texel[..bytes], format, bytes, rect(0, 0, 1, 1)).is_some(),
            "{format:?}"
        );
    }
    assert_eq!(
        area_to_rgba(&[0; 4], Fourcc::NV12, 4, rect(0, 0, 1, 1)),
        None
    );
}

#[test]
fn texels_become_rgba() {
    const ONE: u16 = 0x3c00;
    const HALF: u16 = 0x3800;
    const THIRD: u16 = 0x3555;
    const TWO: u16 = 0x4000;
    const MINUS_ONE: u16 = 0xbc00;
    const TINY: u16 = 0x0001;
    let cases: [(Fourcc, Vec<u8>, [u8; 4]); 12] = [
        (Fourcc::ABGR8888, vec![10, 20, 30, 40], [10, 20, 30, 40]),
        (Fourcc::XBGR8888, vec![10, 20, 30, 40], [10, 20, 30, 255]),
        (Fourcc::ARGB8888, vec![30, 20, 10, 40], [10, 20, 30, 40]),
        (Fourcc::XRGB8888, vec![30, 20, 10, 0], [10, 20, 30, 255]),
        (
            Fourcc::ABGR2101010,
            ten(1023, 512, 0, 2).to_vec(),
            [255, 128, 0, 170],
        ),
        (
            Fourcc::XBGR2101010,
            ten(1023, 512, 0, 0).to_vec(),
            [255, 128, 0, 255],
        ),
        (
            Fourcc::ARGB2101010,
            ten(0, 512, 1023, 3).to_vec(),
            [255, 128, 0, 255],
        ),
        (
            Fourcc::XRGB2101010,
            ten(4, 0, 1023, 0).to_vec(),
            [255, 0, 1, 255],
        ),
        (
            Fourcc::ABGR16161616F,
            halves([ONE, HALF, THIRD, 0]),
            [255, 128, 85, 0],
        ),
        (
            Fourcc::ABGR16161616F,
            halves([TWO, MINUS_ONE, TINY, ONE]),
            [255, 0, 0, 255],
        ),
        (
            Fourcc::XBGR16161616F,
            halves([0, ONE, 0, 0]),
            [0, 255, 0, 255],
        ),
        (
            Fourcc::RGB565,
            0xfc00_u16.to_le_bytes().to_vec(),
            [255, 130, 0, 255],
        ),
    ];
    for (format, texel, rgba) in cases {
        assert_eq!(one(format, &texel), rgba, "{format:?} {texel:?}");
    }
}

#[test]
fn areas_read_through_the_stride() {
    let stride = 20;
    let mut pixels = vec![0u8; stride * 3];
    for (y, x) in [(1usize, 2usize), (1, 3), (2, 2), (2, 3)] {
        let at = y * stride + x * 4;
        pixels[at..at + 4].copy_from_slice(&[x as u8, y as u8, 7, 9]);
    }
    assert_eq!(
        area_to_rgba(&pixels, Fourcc::ARGB8888, stride, rect(2, 1, 2, 2)),
        Some(vec![7, 1, 2, 9, 7, 1, 3, 9, 7, 2, 2, 9, 7, 2, 3, 9])
    );
    for refused in [rect(4, 0, 2, 1), rect(0, 2, 1, 2), rect(-1, 0, 1, 1)] {
        assert_eq!(
            area_to_rgba(&pixels, Fourcc::ARGB8888, stride, refused),
            None,
            "{refused:?}"
        );
    }
}

#[test]
fn damage_stays_sparse_until_it_covers_most_of_its_bounds() {
    let size = Size::from((1000, 1000));
    let mut rects = Vec::new();

    damaged_rects(
        [rect(0, 0, 10, 10), rect(900, 900, 10, 10)],
        size,
        &mut rects,
    );
    assert_eq!(rects, vec![rect(0, 0, 10, 10), rect(900, 900, 10, 10)]);

    damaged_rects(
        [
            rect(0, 0, 10, 10),
            rect(5, 5, 10, 10),
            rect(990, 990, 50, 50),
        ],
        size,
        &mut rects,
    );
    rects.sort_by_key(|rect| rect.loc.x);
    assert_eq!(rects, vec![rect(0, 0, 15, 15), rect(990, 990, 10, 10)]);

    damaged_rects(
        [rect(0, 0, 100, 80), rect(0, 80, 100, 20)],
        size,
        &mut rects,
    );
    assert_eq!(rects, vec![rect(0, 0, 100, 100)]);

    let scattered = (0..17).map(|step| rect(step * 50, step * 50, 4, 4));
    damaged_rects(scattered, size, &mut rects);
    assert_eq!(rects, vec![rect(0, 0, 804, 804)]);

    damaged_rects([rect(2000, 0, 5, 5)], size, &mut rects);
    assert!(rects.is_empty());
}

#[test]
fn rows_are_written_in_the_buffer_format() {
    let rgba: Vec<u8> = (0..2 * 2 * 4).map(|byte| byte as u8).collect();
    let whole = [rect(0, 0, 2, 2)];
    let stride = 12;

    let mut swapped = vec![0xee; stride * 2];
    assert!(write_rows(
        &mut swapped,
        Fourcc::ARGB8888,
        stride,
        (2, 2),
        &rgba,
        whole
    ));
    assert_eq!(swapped[..8], [2, 1, 0, 3, 6, 5, 4, 7]);
    assert_eq!(swapped[8..12], [0xee; 4]);
    assert_eq!(swapped[12..20], [10, 9, 8, 11, 14, 13, 12, 15]);

    let mut opaque = vec![0; stride * 2];
    assert!(write_rows(
        &mut opaque,
        Fourcc::XBGR8888,
        stride,
        (2, 2),
        &rgba,
        whole
    ));
    assert_eq!(opaque[..8], [0, 1, 2, 255, 4, 5, 6, 255]);

    let mut partly = vec![0xee; stride * 2];
    let corner = [rect(1, 1, 5, 5), rect(-3, -3, 1, 1)];
    assert!(write_rows(
        &mut partly,
        Fourcc::ABGR8888,
        stride,
        (2, 2),
        &rgba,
        corner
    ));
    assert_eq!(partly[..16], [0xee; 16]);
    assert_eq!(partly[16..20], [12, 13, 14, 15]);

    let mut short = vec![0; stride];
    assert!(!write_rows(
        &mut short,
        Fourcc::ABGR8888,
        stride,
        (2, 2),
        &rgba,
        whole
    ));
    assert!(!write_rows(
        &mut partly,
        Fourcc::RGB565,
        stride,
        (2, 2),
        &rgba,
        whole
    ));
}

#[test]
fn ten_bit_rows_are_written_in_the_buffer_format() {
    let word: u32 = 0b01 << 30 | 0x155 << 20 | 0x2aa << 10 | 0x3f0;
    let read = word.to_le_bytes();
    let cases = [
        (Fourcc::ABGR2101010, word),
        (
            Fourcc::XBGR2101010,
            0b11 << 30 | 0x155 << 20 | 0x2aa << 10 | 0x3f0,
        ),
        (
            Fourcc::ARGB2101010,
            0b01 << 30 | 0x3f0 << 20 | 0x2aa << 10 | 0x155,
        ),
        (
            Fourcc::XRGB2101010,
            0b11 << 30 | 0x3f0 << 20 | 0x2aa << 10 | 0x155,
        ),
    ];
    for (format, expected) in cases {
        let mut into = [0u8; 4];
        assert!(write_deep_rows(
            &mut into,
            format,
            4,
            (1, 1),
            &read,
            [rect(0, 0, 1, 1)]
        ));
        assert_eq!(u32::from_le_bytes(into), expected, "{format:?}");
    }
    let mut into = [0u8; 4];
    assert!(!write_deep_rows(
        &mut into,
        Fourcc::XRGB8888,
        4,
        (1, 1),
        &read,
        [rect(0, 0, 1, 1)]
    ));
}
