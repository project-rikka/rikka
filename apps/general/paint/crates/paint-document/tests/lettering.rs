use rikka_figure::{Form, Lettering};
use rikka_types::{Color, Rect};

use crate::support::*;
use paint_document::{AddMethod, Error, Figure, FigureEdit, PixelChange};

const INK: [u8; 4] = [0, 0, 255, 255];

fn white(name: &str, width: u32, height: u32) -> Document {
    let pixels = vec![WHITE; (width * height) as usize];
    opened_picture(name, width, &pixels)
}

fn lettering(text: &str) -> Lettering {
    Lettering {
        text: text.into(),
        size: 40.0,
        weight: Lettering::BOLD,
        color: Color::rgba8(INK[0], INK[1], INK[2], INK[3]),
        origin: (10, 10),
        ..Lettering::default()
    }
}

fn lettered(document: &Document, layer: LayerId, lettering: Lettering) -> Edit {
    Edit::Figures {
        layer,
        change: FigureEdit::Add {
            figures: vec![Figure::new(
                document.figure_ids().next(),
                Form::Text(lettering),
            )],
            by: AddMethod::Draw,
        },
    }
}

fn letter(document: &mut Document, lettering: Lettering) -> Change {
    let added = lettered(document, bottom(document), lettering);
    edit(document, added)
}

fn inked(document: &Document) -> Vec<(u32, u32, [u8; 4])> {
    let width = document.width();
    whole_pixels(document)
        .as_chunks::<4>()
        .0
        .iter()
        .enumerate()
        .filter(|(_, pixel)| **pixel != WHITE)
        .map(|(at, pixel)| (at as u32 % width, at as u32 / width, *pixel))
        .collect()
}

fn inside(rect: Rect, x: u32, y: u32) -> bool {
    let (x, y) = (x as f32, y as f32);
    x >= rect.min_x() && x < rect.max_x() && y >= rect.min_y() && y < rect.max_y()
}

#[test]
fn text_renders_inside_measured_bounds() {
    let mut document = white("lettered.png", 240, 80);
    let lettering = lettering("Hello");
    let measure = document.measure_text(&lettering).expect("a measure");

    assert_eq!(
        letter(&mut document, lettering).pixels,
        PixelChange::Content
    );

    let ink = measure.ink.expect("ink");
    let inked = inked(&document);
    assert!(!inked.is_empty());
    assert!(inked.iter().all(|(x, y, _)| inside(ink, *x, *y)));
    assert!(inked.iter().any(|(_, _, pixel)| *pixel == INK));
    assert_eq!((measure.layout.x(), measure.layout.y()), (10.0, 10.0));
    assert_eq!(measure.line_height, 48.0);
    assert!(measure.layout.contains_rect(&ink.inflate(-1.0)));
    assert!(measure.baseline > 10.0 && measure.baseline < 58.0);
}

#[test]
fn invalid_text_is_rejected() {
    let mut document = white("refused.png", 40, 40);
    let bottom = bottom(&document);
    let unknown = Lettering {
        font: "No Such Family Anywhere".into(),
        ..lettering("A")
    };
    let tiny = Lettering {
        size: 0.0,
        ..lettering("A")
    };
    let narrow = Lettering {
        wrap_width: Some(0),
        ..lettering("A")
    };

    let unknown = lettered(&document, bottom, unknown);
    assert!(matches!(
        document.apply(&unknown),
        Err(Error::UnknownFont { .. })
    ));
    assert!(matches!(
        document.measure_text(&tiny),
        Err(Error::Unlettered(_))
    ));
    assert!(matches!(
        document.measure_text(&narrow),
        Err(Error::Unlettered(_))
    ));
    let generic = Lettering {
        font: "serif".into(),
        ..lettering("A")
    };
    assert!(document.measure_text(&generic).is_ok());
}
