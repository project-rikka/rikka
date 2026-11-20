use rikka_psd::{BlendMode, ChannelId};
use rikka_psd_render::{refresh_composite, render_rgba};

use crate::support::*;
use paint_document::{Action, Error, Layer, PixelChange, Properties, Saved, Saving};

const HALF_RED: [u8; 4] = [255, 0, 0, 128];

fn names(layers: &[Layer]) -> Vec<String> {
    layers.iter().map(|layer| layer.name.clone()).collect()
}

fn layer_named(document: &Document, name: &str) -> LayerId {
    fn find(layers: &[Layer], name: &str) -> Option<LayerId> {
        layers.iter().find_map(|layer| match layer.name == name {
            true => Some(layer.id),
            false => find(&layer.children, name),
        })
    }
    find(&document.layers(), name).unwrap_or_else(|| panic!("no layer named {name}"))
}

fn refused(document: &mut Document, edit: Edit) -> Error {
    document.apply(&edit).expect_err("a refused edit")
}

fn stack(layers: Vec<(&str, [u8; 4], BlendMode, u8)>) -> rikka_psd::Document {
    let mut document = rikka_psd::Document::new(2, 2, PsdDepth::Eight, ColorMode::Rgb);
    for (name, color, blend, opacity) in layers {
        let mut record = rikka_psd::Layer::new(name);
        document
            .set_layer_rgba(&mut record, 0, 0, &filled(2, 2, color), Compression::Rle)
            .expect("a layer");
        record.blend_mode = blend;
        record.opacity = opacity;
        document.layers.push(Node::Layer(record));
    }
    document
}

fn with_background(mut document: rikka_psd::Document, color: [u8; 4]) -> rikka_psd::Document {
    let mut paper = rikka_psd::Layer::new("Background");
    document
        .set_layer_rgba(&mut paper, 0, 0, &filled(2, 2, color), Compression::Rle)
        .expect("a layer");
    paper.remove_channel(ChannelId::Transparency);
    document.layers.insert(0, Node::Layer(paper));
    document
}

fn opened_stack(name: &str, document: &rikka_psd::Document) -> Document {
    let mut stored = document.clone();
    refresh_composite(&mut stored).expect("a composite");
    let path = write_fixture(name, &stored.to_bytes().expect("a document"));
    Document::open(&path, &imaging()).expect("an open document")
}

fn rendered(document: &rikka_psd::Document) -> Vec<u8> {
    render_rgba(document).expect("a render").to_rgba8()
}

fn close(left: &[u8], right: &[u8]) {
    assert_eq!(left.len(), right.len());
    for (a, b) in left.iter().zip(right) {
        assert!(a.abs_diff(*b) <= 1, "{left:?} differs from {right:?}");
    }
}

#[test]
fn new_layer_goes_above_active_layer() {
    let mut document = opened_psd("new-layer.psd");
    let paper = layer_named(&document, "paper");

    let made = edit(
        &mut document,
        Edit::NewLayer {
            above: paper,
            name: String::from("Layer 1"),
        },
    );

    assert_eq!(names(&document.layers()), ["drawing", "Layer 1", "paper"]);
    let created = layer_named(&document, "Layer 1");
    assert_eq!(made.layer, Some(created));
    assert!(made.layers && made.history);
    assert_eq!(made.pixels, PixelChange::Content);
    assert_eq!(first_pixel(&document), RED);
    assert_eq!(document.history().actions().last(), Some(&Action::NewLayer));
    assert!(document.thumbnail(created, 4).is_some());

    let undone = edit(&mut document, Edit::Undo);
    assert_eq!(names(&document.layers()), ["drawing", "paper"]);
    assert_eq!(undone.layer, Some(paper));
    let redone = edit(&mut document, Edit::Redo);
    assert_eq!(redone.layer, Some(created));
    assert_eq!(names(&document.layers()), ["drawing", "Layer 1", "paper"]);
}

#[test]
fn layered_image_saves_as_psd() {
    let mut document = opened_picture("round-trip.png", 2, &[RED, BLUE]);
    let bottom = bottom(&document);
    edit(
        &mut document,
        Edit::NewLayer {
            above: bottom,
            name: String::from("ink"),
        },
    );
    let ink = layer_named(&document, "ink");
    edit(
        &mut document,
        Edit::SetProperties {
            layer: ink,
            properties: Properties {
                name: String::from("notes"),
                visible: false,
                opacity: 0.4,
                blend: BlendMode::Screen,
            },
        },
    );
    let path = folder("round-trip").join("layered.psd");

    let saved = document.save(&path, &Saving::default()).expect("a save");

    assert_eq!(saved, Saved::Document);
    let reopened = Document::open(&path, &imaging()).expect("the saved document");
    let layers = reopened.layers();
    assert_eq!(names(&layers), ["notes", "Background"]);
    assert!(!layers[0].visible);
    assert!((layers[0].opacity - 0.4).abs() < 0.01);
    assert_eq!(layers[0].blend, BlendMode::Screen);
    assert!(layers[1].background);
    assert_eq!(whole_pixels(&reopened), [RED, BLUE].concat());
}

#[test]
fn opacity_and_blend_match_psd_render() {
    let source = stack(vec![
        ("paper", BLUE, BlendMode::Normal, 255),
        ("ink", RED, BlendMode::Normal, 255),
    ]);
    let mut document = opened_stack("restyle.psd", &source);
    let ink = layer_named(&document, "ink");

    edit(
        &mut document,
        Edit::SetOpacity {
            layer: ink,
            opacity: 0.5,
        },
    );
    edit(
        &mut document,
        Edit::SetBlend {
            layer: ink,
            blend: BlendMode::Screen,
        },
    );
    let renamed = edit(
        &mut document,
        Edit::RenameLayer {
            layer: ink,
            name: String::from("glow"),
        },
    );

    assert_eq!(renamed.pixels, PixelChange::Unchanged);
    assert!(renamed.layers);
    let expected = stack(vec![
        ("paper", BLUE, BlendMode::Normal, 255),
        ("glow", RED, BlendMode::Screen, 128),
    ]);
    close(&whole_pixels(&document), &rendered(&expected));
    assert_eq!(
        &document.history().actions()[1..],
        [
            Action::LayerOpacity,
            Action::LayerBlend,
            Action::RenameLayer
        ]
    );
    assert!(matches!(
        refused(
            &mut document,
            Edit::SetBlend {
                layer: ink,
                blend: BlendMode::PassThrough,
            },
        ),
        Error::PassThroughOutsideGroup
    ));
    edit(&mut document, Edit::Undo);
    edit(&mut document, Edit::Undo);
    edit(&mut document, Edit::Undo);
    assert_eq!(first_pixel(&document), RED);
}

#[test]
fn merge_down_keeps_composite() {
    let source = with_background(
        stack(vec![
            ("tint", BLUE, BlendMode::Normal, 255),
            ("shade", HALF_RED, BlendMode::Multiply, 200),
        ]),
        WHITE,
    );
    let mut document = opened_stack("merge-down.psd", &source);
    let before = whole_pixels(&document);
    close(&before, &rendered(&source));
    let shade = layer_named(&document, "shade");

    let made = edit(&mut document, Edit::MergeDown { layer: shade });

    assert_eq!(names(&document.layers()), ["tint", "Background"]);
    assert_eq!(made.layer, Some(layer_named(&document, "tint")));
    close(&whole_pixels(&document), &before);
    let tint = layer_named(&document, "tint");
    edit(&mut document, Edit::MergeDown { layer: tint });
    let layers = document.layers();
    assert_eq!(names(&layers), ["Background"]);
    assert!(layers[0].background);
    close(&whole_pixels(&document), &before);
    edit(&mut document, Edit::Undo);
    edit(&mut document, Edit::Undo);
    assert_eq!(document.layers().len(), 3);
    let background = layer_named(&document, "Background");
    assert!(matches!(
        refused(&mut document, Edit::MergeDown { layer: background }),
        Error::NothingBelow
    ));
}
