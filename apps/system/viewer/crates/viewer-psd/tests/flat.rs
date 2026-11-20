use std::path::PathBuf;

use rikka_psd::{
    ColorMode, Compression, Depth, Document, Layer, Node, ResourceData, RgbaImage, VersionInfo,
    resource_id,
};
use viewer_psd::{Error, flatten};

const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];

fn written(name: &str, bytes: &[u8]) -> PathBuf {
    let path = std::env::temp_dir().join(format!("rikka-viewer-psd-{name}"));
    std::fs::write(&path, bytes).expect("writing the document");
    path
}

fn filled(width: u32, height: u32, rgba: [u8; 4]) -> RgbaImage {
    let pixels: Vec<u8> = (0..width * height).flat_map(|_| rgba).collect();
    RgbaImage::from_rgba8(width, height, &pixels).expect("an image")
}

fn painted(merged: bool) -> Document {
    let mut document = Document::new(2, 2, Depth::Eight, ColorMode::Rgb);
    document
        .set_composite_rgba(&filled(2, 2, BLUE), Compression::Rle)
        .expect("a composite");
    let mut layer = Layer::new("ink");
    document
        .set_layer_rgba(&mut layer, 0, 0, &filled(2, 2, RED), Compression::Rle)
        .expect("a layer");
    document.layers.push(Node::Layer(layer));
    document.resources.set(
        resource_id::VERSION_INFO,
        ResourceData::VersionInfo(VersionInfo {
            version: 1,
            has_real_merged_data: merged,
            writer: String::from("Adobe Photoshop"),
            reader: String::from("Adobe Photoshop CC"),
            file_version: 1,
        }),
    );
    document
}

#[test]
fn uses_stored_composite() {
    let mut document = Document::new(3, 1, Depth::Sixteen, ColorMode::Rgb);
    let saved = [255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 0, 0];
    let image = RgbaImage::from_rgba8(3, 1, &saved).expect("an image");
    document
        .set_composite_rgba(&image, Compression::Rle)
        .expect("a composite");
    let path = written("stored.psd", &document.to_bytes().expect("a document"));

    let flat = flatten(&path).expect("a stored composite flattens");

    assert_eq!((flat.width, flat.height), (3, 1));
    for (rendered, saved) in flat.pixels.iter().zip(saved) {
        assert!(rendered.abs_diff(saved) <= 1, "{:?}", flat.pixels);
    }
}

#[test]
fn composites_layers_without_stored_composite() {
    let path = written("merged.psd", &painted(true).to_bytes().expect("a document"));
    let flat = flatten(&path).expect("the saved composite");
    assert_eq!(flat.pixels[..4], BLUE);

    let path = written(
        "layered.psd",
        &painted(false).to_bytes().expect("a document"),
    );
    let flat = flatten(&path).expect("the layers composited");
    assert_eq!(flat.pixels.len(), 16);
    assert!(
        flat.pixels
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| *pixel == RED)
    );
}

#[test]
fn rejects_oversized_document_before_decoding() {
    let mut header = b"8BPS\x00\x01\0\0\0\0\0\0\x00\x04".to_vec();
    header.extend_from_slice(&30_000_u32.to_be_bytes());
    header.extend_from_slice(&30_000_u32.to_be_bytes());
    header.extend_from_slice(&[0, 16, 0, 4]);
    let path = written("huge.psd", &header);

    assert!(matches!(
        flatten(&path),
        Err(Error::TooLarge {
            width: 30_000,
            height: 30_000
        })
    ));
}
