use rikka_image_edit::{Brush, BrushStroke, Ink, Marks, Operation, Recipe};
use rikka_types::{Color, Point};

use crate::support::*;
use paint_document::{Action, PixelChange};

fn grounded(name: &str) -> Document {
    let mut document = rikka_psd::Document::new(8, 8, PsdDepth::Eight, ColorMode::Rgb);
    let mut background = rikka_psd::Layer::new("Background");
    document
        .set_layer_rgba(&mut background, 0, 0, &filled(8, 8, RED), Compression::Rle)
        .expect("a layer");
    background.remove_channel(rikka_psd::ChannelId::Transparency);
    let mut top = rikka_psd::Layer::new("top");
    document
        .set_layer_rgba(&mut top, 0, 0, &filled(8, 8, BLUE), Compression::Rle)
        .expect("a layer");
    document.layers.push(Node::Layer(background));
    document.layers.push(Node::Layer(top));
    rikka_psd_render::refresh_composite(&mut document).expect("a composite");
    let path = write_fixture(name, &document.to_bytes().expect("a document"));
    Document::open(&path, &imaging()).expect("an open document")
}

fn across(brush: Brush, ink: Ink, y: f32, width: f32) -> Recipe {
    let stroke = BrushStroke::from_points(brush, ink, [Point::new(0.0, y), Point::new(width, y)])
        .expect("stroke has points");
    Recipe::new().then(Marks::new([stroke]))
}

fn eraser(background: Color) -> Ink {
    Ink::Erase { background }
}

#[test]
fn brush_stroke_is_one_history_entry() {
    let mut document = opened_picture("brush.png", 8, &[WHITE; 64]);
    let bottom = bottom(&document);

    let change = develop(
        &mut document,
        bottom,
        across(Brush::new(3.0), Ink::Paint(Color::BLACK), 4.5, 8.0),
    );

    assert_eq!(change.pixels, PixelChange::Content);
    assert_eq!(pixel_at(&document, 3, 4), [0, 0, 0, 255]);
    assert_eq!(pixel_at(&document, 3, 0), WHITE);
    assert!(matches!(
        document.history().actions(),
        [Action::Open, Action::Develop(Operation::Marks(_))]
    ));
    edit(&mut document, Edit::Undo);
    assert_eq!(pixel_at(&document, 3, 4), WHITE);
    edit(&mut document, Edit::Redo);
    assert_eq!(pixel_at(&document, 3, 4), [0, 0, 0, 255]);
}

#[test]
fn eraser_clears_to_layers_below() {
    let mut document = grounded("erase-layer.psd");
    let top = document.layers()[0].id;

    develop(
        &mut document,
        top,
        across(Brush::new(3.0), eraser(Color::WHITE), 4.5, 8.0),
    );

    assert_eq!(pixel_at(&document, 3, 4), RED);
    assert_eq!(pixel_at(&document, 3, 0), BLUE);
    edit(&mut document, Edit::Undo);
    assert_eq!(pixel_at(&document, 3, 4), BLUE);
}
