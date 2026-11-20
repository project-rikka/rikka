use rikka_image_edit::{Intent, Profile, QuarterTurn, Resampling};
use rikka_psd::{ResourceData, resource_id};

use crate::support::*;
use paint_document::{Area, Error, PixelChange, Reshape};

fn reshape(document: &mut Document, reshape: Reshape) -> Change {
    edit(document, Edit::Reshape(reshape))
}

const GREEN: [u8; 4] = [0, 255, 0, 255];

#[test]
fn resize_resamples_and_clears_selection() {
    let mut document = opened_picture("resize.png", 2, &[RED; 4]);
    select(&mut document, Region::rectangle(Area::new(0, 0, 1, 1)));

    let change = reshape(
        &mut document,
        Reshape::Resize {
            width: 4,
            height: 6,
            resampling: Resampling::Bilinear,
        },
    );

    assert_eq!(change.pixels, PixelChange::Canvas);
    assert!(change.selection);
    assert_eq!((document.width(), document.height()), (4, 6));
    assert_eq!(pixel_at(&document, 3, 5), RED);
    assert!(document.selection().is_empty());
    edit(&mut document, Edit::Undo);
    assert_eq!((document.width(), document.height()), (2, 2));
    assert_eq!(document.selection().bounds(), Some(Area::new(0, 0, 1, 1)));
}

#[test]
fn psd_conversion_embeds_profile() {
    let mut document = opened_psd("convert.psd");
    let p3 = Profile::display_p3();

    edit(
        &mut document,
        Edit::Convert {
            profile: p3.clone(),
            intent: Intent::Perceptual,
        },
    );

    assert_eq!(document.profile(), Some(&p3));
    let path = folder("convert-psd").join("converted.psd");
    document
        .save(&path, &paint_document::Saving::default())
        .expect("a save");
    let saved =
        rikka_psd::Document::read(&std::fs::read(&path).expect("bytes")).expect("a document");
    assert_eq!(saved.resources.icc_profile(), Some(&p3.icc()[..]));
    assert_ne!(saved.composite_rgba8().expect("pixels")[..4], RED);
}

#[test]
fn only_rgb_psd_converts_profile() {
    let mut gray = rikka_psd::Document::new(
        2,
        2,
        rikka_psd::Depth::Eight,
        rikka_psd::ColorMode::Grayscale,
    );
    gray.set_composite_rgba(&filled(2, 2, WHITE), rikka_psd::Compression::Rle)
        .expect("a composite");
    gray.resources.set(
        resource_id::VERSION_INFO,
        ResourceData::VersionInfo(rikka_psd::VersionInfo {
            version: 1,
            has_real_merged_data: true,
            writer: String::new(),
            reader: String::new(),
            file_version: 1,
        }),
    );
    let path = write_fixture("gray.psd", &gray.to_bytes().expect("a document"));
    let mut document = Document::open(&path, &imaging()).expect("an open document");

    let refused = document.apply(&Edit::Convert {
        profile: Profile::display_p3(),
        intent: Intent::Perceptual,
    });

    assert!(matches!(refused, Err(Error::ProfileNeedsRgb)));
    assert!(!document.is_modified());
}

fn patterned(name: &str) -> Document {
    let mut document =
        rikka_psd::Document::new(3, 2, rikka_psd::Depth::Eight, rikka_psd::ColorMode::Rgb);
    let ground = [RED, BLUE, BLUE, GREEN, BLUE, BLUE].concat();
    let ground = rikka_psd::RgbaImage::from_rgba8(3, 2, &ground).expect("an image");
    let mut background = rikka_psd::Layer::new("Background");
    document
        .set_layer_rgba(&mut background, 0, 0, &ground, rikka_psd::Compression::Rle)
        .expect("a layer");
    background.remove_channel(rikka_psd::ChannelId::Transparency);
    let mut dot = rikka_psd::Layer::new("dot");
    document
        .set_layer_rgba(
            &mut dot,
            2,
            1,
            &filled(1, 1, WHITE),
            rikka_psd::Compression::Rle,
        )
        .expect("a layer");
    document.layers.push(rikka_psd::Node::Layer(background));
    document.layers.push(rikka_psd::Node::Layer(dot));
    rikka_psd_render::refresh_composite(&mut document).expect("a composite");
    let path = write_fixture(name, &document.to_bytes().expect("a document"));
    Document::open(&path, &imaging()).expect("an open document")
}

#[test]
fn psd_rotation_rotates_every_layer() {
    let mut document = patterned("turn-psd.psd");

    let change = reshape(&mut document, Reshape::Turn(QuarterTurn::Clockwise));

    assert_eq!(change.pixels, PixelChange::Canvas);
    assert_eq!((document.width(), document.height()), (2, 3));
    assert_eq!(pixel_at(&document, 0, 0), GREEN);
    assert_eq!(pixel_at(&document, 1, 0), RED);
    assert_eq!(pixel_at(&document, 0, 2), WHITE);
    assert!(document.layers()[1].background);
    edit(&mut document, Edit::Undo);
    assert_eq!((document.width(), document.height()), (3, 2));
    assert_eq!(first_pixel(&document), RED);
}
