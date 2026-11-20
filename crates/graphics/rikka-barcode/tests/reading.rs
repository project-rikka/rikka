use rikka_barcode::{Barcode, Error, Format, Picture, QrCode, read, read_framed};
use rikka_types::Rect;

use crate::support::{Canvas, INK, PAPER};

const GREY: [u8; 4] = [168, 172, 176, 255];

fn picture(canvas: &Canvas) -> Picture<'_> {
    Picture::rgba(canvas.width, canvas.height, &canvas.pixels).expect("valid canvas")
}

fn find_by_text<'a>(barcodes: &'a [Barcode], text: &str) -> &'a Barcode {
    barcodes
        .iter()
        .find(|barcode| barcode.text() == text)
        .unwrap_or_else(|| panic!("`{text}` not found in {barcodes:?}"))
}

fn assert_near(bounds: Rect, expected: Rect, slack: f32) {
    let edges = [
        (bounds.min_x(), expected.min_x()),
        (bounds.min_y(), expected.min_y()),
        (bounds.max_x(), expected.max_x()),
        (bounds.max_y(), expected.max_y()),
    ];
    for (edge, target) in edges {
        assert!(
            (edge - target).abs() <= slack,
            "bounds {bounds:?} differ from drawn {expected:?}"
        );
    }
}

#[test]
fn reads_every_code_in_scene() {
    let written = "こんにちは、世界 — https://example.com/q?a=1&b=2";
    let darkened = "WIFI:T:WPA;S:Rikka;P:hunter2;;";
    let mut canvas = Canvas::filled(960, 640, GREY);

    canvas.fill((60, 60), (300, 300), PAPER);
    let side = canvas.code(written, (80, 80), 4, false);
    let first = Rect::new(80.0, 80.0, side as f32, side as f32);

    canvas.fill((560, 280), (200, 200), INK);
    let side = canvas.code(darkened, (580, 300), 3, true);
    let second = Rect::new(580.0, 300.0, side as f32, side as f32);

    let barcodes = read(&picture(&canvas)).expect("scene decodes");

    assert_eq!(barcodes.len(), 2, "{barcodes:?}");
    for (text, drawn, module) in [(written, first, 4.0), (darkened, second, 3.0)] {
        let barcode = find_by_text(&barcodes, text);
        assert_eq!(barcode.format(), Format::QrCode);
        assert_near(barcode.bounds(), drawn, module * 1.5);
    }
}

#[test]
fn reads_code_without_quiet_zone() {
    let text = "https://rikka.example/scan";
    for inverted in [false, true] {
        let (paper, module) = match inverted {
            true => (INK, 2),
            false => (PAPER, 2),
        };
        let mut canvas = Canvas::filled(200, 200, paper);
        let side = canvas.code(text, (40, 40), module, inverted);
        let tight = canvas.crop((40, 40), (side, side));

        let barcodes = read_framed(&picture(&tight)).expect("frame decodes");

        let barcode = find_by_text(&barcodes, text);
        assert_near(
            barcode.bounds(),
            Rect::new(0.0, 0.0, side as f32, side as f32),
            module as f32 * 1.5,
        );
    }
}

#[test]
fn rasterized_code_reads_back() {
    let text = "WIFI:T:WPA;S:Rikka;P:hunter2;;";
    let code = QrCode::encode(text).expect("text fits in QR code");
    assert_eq!(code.side(), 29, "medium error correction");

    let image = code.rasterize(3);
    assert_eq!(image.side(), (29 + 8) * 3, "quiet zone on every side");
    let picture = Picture::rgba(image.side(), image.side(), image.rgba()).expect("valid image");
    let barcodes = read(&picture).expect("image decodes");

    assert_eq!(find_by_text(&barcodes, text).format(), Format::QrCode);
}

#[test]
fn rejects_mismatched_pixel_data() {
    assert!(matches!(Picture::rgba(0, 4, &[]), Err(Error::Empty)));
    assert!(matches!(
        Picture::rgba(2, 2, &[0; 15]),
        Err(Error::Mismatched { length: 15, .. })
    ));
}
