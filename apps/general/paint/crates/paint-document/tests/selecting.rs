use rikka_types::Color;

use crate::support::*;
use paint_document::{Action, Area, Combine, Error, Flood, FloodMode, PixelChange, Shape};

const GREEN: [u8; 4] = [0, 255, 0, 255];

fn plain(name: &str, width: u32, height: u32) -> Document {
    let pixels: Vec<[u8; 4]> = (0..width * height).map(|_| RED).collect();
    opened_picture(name, width, &pixels)
}

fn halves(name: &str) -> Document {
    let pixels: Vec<[u8; 4]> = (0..4)
        .flat_map(|_| (0..8).map(|x| if x < 4 { RED } else { BLUE }))
        .collect();
    opened_picture(name, 8, &pixels)
}

fn shape(document: &mut Document, shape: Shape, combine: Combine) -> Change {
    edit(document, Edit::SelectShape { shape, combine })
}

fn last_action(document: &Document) -> Action {
    let history = document.history();
    history.actions()[history.current()].clone()
}

fn selection_at(document: &Document, x: u32, y: u32) -> u8 {
    document.selection().coverage(x, y)
}

#[test]
fn shapes_combine_with_selection_in_every_mode() {
    let mut document = plain("select-shapes.png", 20, 20);

    let change = shape(
        &mut document,
        Shape::rectangle(0.0, 0.0, 10.0, 20.0),
        Combine::Replace,
    );
    assert_eq!(
        change,
        Change {
            pixels: PixelChange::Unchanged,
            selection: true,
            history: true,
            ..Change::NONE
        }
    );
    shape(
        &mut document,
        Shape::ellipse(6.0, 6.0, 10.0, 8.0),
        Combine::Union,
    );
    assert_eq!(
        last_action(&document),
        Action::Select(SelectionAction::Ellipse)
    );
    assert_eq!(selection_at(&document, 14, 10), 255);
    assert_eq!(selection_at(&document, 18, 10), 0);
    let union = document.selection().clone();

    shape(
        &mut document,
        Shape::rectangle(0.0, 0.0, 20.0, 5.0),
        Combine::Exclude,
    );
    assert_eq!(selection_at(&document, 2, 2), 0);
    assert_eq!(selection_at(&document, 2, 8), 255);
    shape(
        &mut document,
        Shape::rectangle(0.0, 0.0, 3.0, 20.0),
        Combine::Intersect,
    );
    assert_eq!(document.selection().bounds(), Some(Area::new(0, 5, 3, 15)));
    shape(
        &mut document,
        Shape::rectangle(0.0, 5.0, 5.0, 15.0),
        Combine::Xor,
    );
    assert_eq!(document.selection().bounds(), Some(Area::new(3, 5, 2, 15)));

    edit(&mut document, Edit::Undo);
    edit(&mut document, Edit::Undo);
    edit(&mut document, Edit::Undo);
    assert_eq!(*document.selection(), union);
    let redone = edit(&mut document, Edit::Redo);
    assert!(redone.selection && redone.pixels == PixelChange::Unchanged);
    assert_eq!(selection_at(&document, 2, 2), 0);
}

#[test]
fn magic_wand_global_and_contiguous() {
    let mut document = opened_picture(
        "select-wand.png",
        6,
        &[
            RED, RED, BLUE, RED, RED, BLUE, RED, RED, BLUE, RED, RED, BLUE,
        ],
    );
    let wand = Flood::from_point(0.5, 0.5).anti_aliased(false);

    edit(
        &mut document,
        Edit::MagicWand {
            flood: wand,
            combine: Combine::Replace,
        },
    );
    assert_eq!(
        last_action(&document),
        Action::Select(SelectionAction::MagicWand)
    );
    assert_eq!(document.selection().rectangular(), Some(Area::sized(2, 2)));

    edit(
        &mut document,
        Edit::MagicWand {
            flood: wand.mode(FloodMode::Global),
            combine: Combine::Replace,
        },
    );
    assert_eq!(selection_at(&document, 3, 1), 255);
    assert_eq!(selection_at(&document, 2, 1), 0);

    edit(
        &mut document,
        Edit::MagicWand {
            flood: Flood::from_point(2.0, 0.0).anti_aliased(false),
            combine: Combine::Union,
        },
    );
    assert_eq!(selection_at(&document, 2, 1), 255);
    assert_eq!(selection_at(&document, 5, 1), 0);
}

#[test]
fn paint_bucket_fills_within_selection() {
    let mut document = halves("bucket-plain.png");
    let bottom = bottom(&document);
    shape(
        &mut document,
        Shape::rectangle(0.0, 0.0, 2.0, 4.0),
        Combine::Replace,
    );

    let change = edit(
        &mut document,
        Edit::Bucket {
            layer: bottom,
            flood: Flood::from_point(0.0, 0.0).anti_aliased(false),
            color: Color::rgba8(0, 255, 0, 255),
        },
    );

    assert_eq!(change.pixels, PixelChange::Content);
    assert_eq!(last_action(&document), Action::Bucket);
    assert_eq!(pixel_at(&document, 1, 2), GREEN);
    assert_eq!(pixel_at(&document, 3, 2), RED);
    assert_eq!(pixel_at(&document, 5, 2), BLUE);
    edit(&mut document, Edit::Undo);
    assert_eq!(pixel_at(&document, 1, 2), RED);
    edit(&mut document, Edit::Redo);
    assert_eq!(pixel_at(&document, 1, 2), GREEN);
}

#[test]
fn seed_outside_canvas_is_rejected() {
    let mut document = halves("bucket-off.png");
    let bottom = bottom(&document);

    let refused = document.apply(&Edit::Bucket {
        layer: bottom,
        flood: Flood::from_point(8.0, 0.0),
        color: Color::WHITE,
    });

    assert!(matches!(refused, Err(Error::OutsideImage { .. })));
    assert_eq!(document.history().actions().len(), 1);
}

const CLEAR: [u8; 4] = [0, 0, 0, 0];

fn bytes(color: Color) -> [u8; 4] {
    color.to_rgba8()
}

#[test]
fn transparent_pixels_do_not_darken_sample() {
    let mut document = opened_picture("samples-clear.png", 2, &[RED, CLEAR, CLEAR, CLEAR]);

    let color = document.sample(0, 0, 3, None).expect("a colour");

    let [r, g, b, a] = bytes(color);
    assert_eq!((r, g, b, a), (255, 0, 0, 64));
}

#[test]
fn sample_outside_canvas_is_rejected() {
    let mut document = opened_picture("samples-outside.png", 1, &[RED]);

    assert!(document.sample(1, 0, 1, None).is_err());
}
