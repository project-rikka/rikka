use std::io::Cursor;

use rikka_png::{Layout, probe};

use crate::support::{assemble, chunks, header, rgba_png, texture, zlib};

#[test]
fn probe_reads_size_layout_and_every_text_form_without_the_pixels() {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 5, 4);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Sixteen);
        encoder
            .add_text_chunk("Thumb::URI".into(), "file:///caf\u{e9}".into())
            .expect("adds text");
        encoder
            .add_ztxt_chunk("Comment".into(), "squeezed ".repeat(40))
            .expect("adds compressed text");
        encoder
            .add_itxt_chunk("Title".into(), "\u{5199}\u{771f}".into())
            .expect("adds international text");
        let mut writer = encoder.write_header().expect("writes header");
        writer
            .write_image_data(&texture(5 * 4 * 6, 4))
            .expect("writes pixels");
        writer.finish().expect("finishes");
    }

    let info = probe(Cursor::new(&bytes)).expect("probes");

    assert_eq!((info.width, info.height), (5, 4));
    assert_eq!(info.layout, Layout::Rgb16);
    assert!(!info.animated);
    let text: Vec<(&str, &str)> = info
        .text
        .iter()
        .map(|entry| (entry.keyword.as_str(), entry.text.as_str()))
        .collect();
    let squeezed = "squeezed ".repeat(40);
    assert_eq!(
        text,
        [
            ("Thumb::URI", "file:///caf\u{e9}"),
            ("Comment", squeezed.as_str()),
            ("Title", "\u{5199}\u{771f}"),
        ]
    );

    let header_only = &bytes[..8 + 12 + 13];
    let cut = probe(Cursor::new(header_only)).expect("header is enough");
    assert_eq!((cut.width, cut.height), (5, 4));
}

#[test]
fn probe_sees_transparency_keys_orientation_data_and_animation() {
    let exif = b"II*\0\x08\0\0\0".to_vec();
    let keyed = assemble(&[
        (b"IHDR", header(2, 2, 8, 3, 0)),
        (b"PLTE", vec![0; 6]),
        (b"tRNS", vec![0]),
        (b"acTL", vec![0, 0, 0, 1, 0, 0, 0, 0]),
        (b"IDAT", zlib(&[0, 0, 1, 0, 1, 0])),
        (b"eXIf", exif.clone()),
        (b"IEND", Vec::new()),
    ]);

    let info = probe(Cursor::new(&keyed)).expect("probes");

    assert_eq!(info.layout, Layout::Rgba8);
    assert!(info.animated);
    assert_eq!(info.exif, Some(exif));

    let plain = rgba_png(3, 1, &[0; 12]);
    let parts = chunks(&plain);
    assert_eq!(parts[0].0, *b"IHDR");
    let info = probe(Cursor::new(&plain)).expect("probes");
    assert_eq!(info.layout, Layout::Rgba8);
    assert!(!info.animated && info.exif.is_none() && info.text.is_empty());
}
