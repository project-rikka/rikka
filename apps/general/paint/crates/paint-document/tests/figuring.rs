use rikka_figure::{Form, Frame, Look, Shape};
use rikka_types::{Color, Point};

use crate::support::*;
use paint_document::{AddMethod, Figure, FigureChange, FigureEdit};

fn white(name: &str, width: u32, height: u32) -> Document {
    opened_picture(name, width, &vec![WHITE; (width * height) as usize])
}

fn block(document: &Document, rect: (f32, f32, f32, f32), color: [u8; 4]) -> Figure {
    let (x, y, width, height) = rect;
    Figure::new(
        document.figure_ids().next(),
        Form::Shape {
            shape: Shape::Rectangle { radius: 0.0 },
            frame: Frame::from_corners(Point::new(x, y), Point::new(x + width, y + height)),
            look: Look {
                fill: Some(Color::rgba8(color[0], color[1], color[2], color[3])),
                outline: None,
            },
        },
    )
}

fn add(document: &mut Document, layer: LayerId, figures: Vec<Figure>) -> Change {
    edit(
        document,
        Edit::Figures {
            layer,
            change: FigureEdit::Add {
                figures,
                by: AddMethod::Draw,
            },
        },
    )
}

#[test]
fn undo_restores_shape_and_pixels() {
    let mut document = white("undone.png", 20, 20);
    let bottom = bottom(&document);
    let before = whole_pixels(&document);
    let square = block(&document, (2.0, 2.0, 10.0, 10.0), RED);
    add(&mut document, bottom, vec![square.clone()]);
    let drawn = whole_pixels(&document);

    edit(&mut document, Edit::Undo);
    assert_eq!(whole_pixels(&document), before);
    assert!(document.layers()[0].figures.is_empty());

    edit(&mut document, Edit::Redo);
    assert_eq!(whole_pixels(&document), drawn);
    assert_eq!(document.layers()[0].figures, vec![square]);
}

#[test]
fn conflicting_or_missing_objects_are_rejected() {
    let mut document = white("refused.png", 20, 20);
    let bottom = bottom(&document);
    let square = block(&document, (2.0, 2.0, 10.0, 10.0), RED);
    add(&mut document, bottom, vec![square.clone()]);

    let again = Edit::Figures {
        layer: bottom,
        change: FigureEdit::Add {
            figures: vec![square.clone()],
            by: AddMethod::Paste,
        },
    };
    assert!(document.apply(&again).is_err());
    let stranger = block(&document, (0.0, 0.0, 4.0, 4.0), RED);
    let missing = Edit::Figures {
        layer: bottom,
        change: FigureEdit::Change {
            figures: vec![stranger],
            by: FigureChange::Restyle,
        },
    };
    assert!(document.apply(&missing).is_err());
    assert_eq!(document.history().actions().len(), 2);
}

#[test]
fn save_rasterizes_shapes() {
    let mut document = white("saved.png", 20, 20);
    let bottom = bottom(&document);
    let square = block(&document, (2.0, 2.0, 10.0, 10.0), RED);
    add(&mut document, bottom, vec![square]);
    let path = folder("figured").join("saved.png");

    document
        .save(&path, &paint_document::Saving::default())
        .expect("a save");

    let reopened = Document::open(&path, &imaging()).expect("the saved picture");
    assert_eq!(pixel_at(&reopened, 6, 6), RED);
    assert!(reopened.layers()[0].figures.is_empty());
    assert_eq!(document.layers()[0].figures.len(), 1);
}
