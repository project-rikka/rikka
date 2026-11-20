use rikka_image::{ColorType, Image};
use rikka_image_edit::{GaussianBlur, Invert, Mask, Recipe, Step};

use crate::support::*;
use paint_document::{Area, Error, FileFormat, LayerKind, PixelChange, Saving, preview};

fn crop(document: &mut Document, region: Region) -> Change {
    edit(document, Edit::Crop(region))
}

#[test]
fn psd_lists_layers_top_first() {
    let document = opened_psd("layers.psd");

    let layers = document.layers();

    let names: Vec<&str> = layers.iter().map(|layer| layer.name.as_str()).collect();
    assert_eq!(names, ["drawing", "paper"]);
    assert_eq!(layers[0].kind, LayerKind::Group);
    assert_eq!(layers[0].children[0].name, "ink");
    assert!(matches!(
        document.format(),
        FileFormat::Photoshop { large: false }
    ));
}

#[test]
fn non_image_is_unsupported() {
    let path = write_fixture("notes.txt", b"just some words");

    assert!(matches!(
        Document::open(&path, &imaging()),
        Err(Error::Unsupported { .. })
    ));
}

#[test]
fn oversized_document_is_rejected_early() {
    let mut header = b"8BPS\x00\x01\0\0\0\0\0\0\x00\x04".to_vec();
    header.extend_from_slice(&30_000_u32.to_be_bytes());
    header.extend_from_slice(&30_000_u32.to_be_bytes());
    header.extend_from_slice(&[0, 16, 0, 4]);
    let path = write_fixture("huge.psd", &header);

    assert!(matches!(
        Document::open(&path, &imaging()),
        Err(Error::TooLarge {
            width: 30_000,
            height: 30_000
        })
    ));
}

#[test]
fn psd_crop_keeps_layers() {
    let mut document = opened_psd("crop.psd");

    let change = crop(&mut document, Region::rectangle(Area::new(1, 0, 1, 2)));

    assert_eq!(change.pixels, PixelChange::Canvas);
    assert_eq!((document.width(), document.height()), (1, 2));
    assert_eq!(whole_pixels(&document), [RED, RED].concat());
    assert_eq!(document.layers().len(), 2);
}

#[test]
fn recipe_applies_within_selection() {
    let mut document = opened_picture("develop.png", 2, &[[100, 100, 100, 255]; 4]);
    let selection = Region::rectangle(Area::new(0, 0, 1, 2))
        .mask(2, 2)
        .expect("a mask");
    let recipe = Recipe::new().then(Step::new(Invert).masked(selection));

    let bottom = bottom(&document);
    let change = develop(&mut document, bottom, recipe);

    assert_eq!(change.pixels, PixelChange::Content);
    let pixels = whole_pixels(&document);
    assert_eq!(pixels[..4], [155, 155, 155, 255]);
    assert_eq!(pixels[4..8], [100, 100, 100, 255]);
}

#[test]
fn save_format_follows_extension() {
    let mut document = opened_picture("save.png", 2, &[RED, BLUE]);
    let bottom = bottom(&document);
    develop(
        &mut document,
        bottom,
        Recipe::new()
            .then(Step::new(GaussianBlur::new(0.0)).masked(Mask::rectangle(0.0, 0.0, 1.0, 1.0))),
    );
    let path = folder("save").join("saved.png");

    document
        .save(&path, &Saving::default())
        .expect("a saved picture");

    let saved = Image::open(&path).expect("the saved picture");
    assert_eq!(
        saved.convert(ColorType::Rgba8).as_bytes(),
        [RED, BLUE].concat()
    );
}

fn covered(tiles: &[paint_document::Tile], width: u32, height: u32) -> bool {
    let mut visits = vec![0u32; (width * height) as usize];
    for tile in tiles {
        let area = tile.area;
        for y in area.top..area.top + area.height {
            for x in area.left..area.right() {
                visits[(y * width + x) as usize] += 1;
            }
        }
    }
    visits.iter().all(|count| *count == 1)
}

fn shades(width: u32, height: u32) -> Vec<[u8; 4]> {
    (0..width * height)
        .map(|index| [(index * 7 % 256) as u8, (index * 3 % 256) as u8, 90, 255])
        .collect()
}

#[test]
fn reduced_level_tiles_cover_document() {
    let mut document = opened_picture("reduced.png", 20, &shades(20, 12));

    for (level, side, frame_width) in [(0, 8, 8), (1, 8, 8), (2, 4, 4)] {
        let tiles = document.tiles(side, level).expect("tiles");

        assert!(covered(&tiles, 20, 12), "level {level} leaves a gap");
        assert_eq!(tiles[0].frame.width(), frame_width, "level {level}");
        assert_eq!(tiles[0].area.width, frame_width << level, "level {level}");
    }
    assert_eq!(paint_document::level_for(0.3), 1);
    assert_eq!(paint_document::level_for(1.5), 0);
}

fn reopened_thumbnail(document: &Document, name: &str, longest: u32) -> Vec<u8> {
    let pixels = whole_pixels(document);
    let texels = pixels.as_chunks::<4>().0.to_vec();
    let mut reopened = opened_picture(name, document.width(), &texels);
    let layer = bottom(&reopened);
    reopened
        .thumbnail(layer, longest)
        .expect("a thumbnail")
        .pixels
}

#[test]
fn thumbnail_tracks_edits_and_undo() {
    let mut document = opened_picture("followed.png", 160, &shades(160, 120));
    let layer = bottom(&document);
    let original = document.thumbnail(layer, 40).expect("a thumbnail").pixels;
    let inverted =
        Recipe::new().then(Step::new(Invert).masked(Mask::ellipse(70.0, 40.0, 30.0, 20.0)));

    edit(
        &mut document,
        Edit::Develop {
            layer,
            recipe: inverted,
        },
    );
    let edited = document.thumbnail(layer, 40).expect("a thumbnail").pixels;

    assert_ne!(edited, original);
    assert_eq!(edited, reopened_thumbnail(&document, "edited.png", 40));
    edit(&mut document, Edit::Undo);
    assert_eq!(
        document.thumbnail(layer, 40).expect("a thumbnail").pixels,
        original
    );
    edit(&mut document, Edit::Redo);
    assert_eq!(
        document.thumbnail(layer, 40).expect("a thumbnail").pixels,
        edited
    );
}

#[test]
fn preview_does_not_modify_document() {
    let mut document = opened_picture("preview-full.png", 16, &shades(16, 12));
    let bottom = bottom(&document);
    let before = whole_pixels(&document);
    let selection = Region::rectangle(Area::new(2, 3, 9, 5));
    let recipe = Recipe::new()
        .then(GaussianBlur::new(1.5))
        .then(Step::new(Invert).masked(selection.mask(16, 12).expect("a mask")));

    let tiles = document
        .preview(bottom, &recipe, 1.0)
        .expect("preview tiles");

    assert!(covered(&tiles, 16, 12));
    assert!(!document.is_modified());
    assert_eq!(whole_pixels(&document), before);
}

#[test]
fn psd_preview_uses_stored_thumbnail() {
    let mut stored = rikka_psd::Document::new(400, 200, PsdDepth::Eight, ColorMode::Rgb);
    let rgb: Vec<u8> = (0..4 * 2).flat_map(|_| [0, 128, 255]).collect();
    stored.resources.set(
        resource_id::THUMBNAIL,
        ResourceData::Thumbnail(rikka_psd::Thumbnail::from_rgb8(4, 2, &rgb).expect("a thumbnail")),
    );
    let path = write_fixture("stored.psd", &stored.to_bytes().expect("a document"));

    let thumbnail = preview(&path, 64).expect("a preview");

    assert_eq!((thumbnail.width, thumbnail.height), (4, 2));
    assert_eq!(thumbnail.pixels[..4], [0, 128, 255, 255]);
}

#[test]
fn preview_fits_requested_size() {
    let pixels: Vec<u8> = (0..300 * 100).flat_map(|_| [10, 20, 30, 255]).collect();
    let png = Image::from_rgba8(300, 100, pixels)
        .expect("an image")
        .write(rikka_image::Png)
        .to_bytes()
        .expect("a png");
    let path = write_fixture("wide.png", &png);

    let thumbnail = preview(&path, 60).expect("a preview");

    assert!(thumbnail.width <= 60 && thumbnail.height <= 60);
    assert_eq!(thumbnail.width, 60);
}
