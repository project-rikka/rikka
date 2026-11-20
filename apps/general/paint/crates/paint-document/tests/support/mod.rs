use rikka_sync::{Mutex, MutexGuard};
use std::path::PathBuf;

pub(crate) use paint_document::{Change, Document, Edit, LayerId, Region, SelectionAction};
pub(crate) use rikka_image::{ColorType, Image};
pub(crate) use rikka_image_edit::{Imaging, Recipe};
pub(crate) use rikka_psd::{
    ColorMode, Compression, Depth as PsdDepth, Group, Node, ResourceData, RgbaImage, VersionInfo,
    resource_id,
};

pub(crate) const RED: [u8; 4] = [255, 0, 0, 255];
pub(crate) const BLUE: [u8; 4] = [0, 0, 255, 255];
pub(crate) const WHITE: [u8; 4] = [255, 255, 255, 255];

static TURN: Mutex<()> = Mutex::new(());

struct Engine {
    imaging: Imaging,
    _turn: MutexGuard<'static, ()>,
}

thread_local! {
    static ENGINE: Engine = {
        let turn = TURN.lock();
        Engine {
            imaging: Imaging::headless().expect("a Vulkan 1.3 device that can compute"),
            _turn: turn,
        }
    };
}

pub(crate) fn imaging() -> Imaging {
    ENGINE.with(|engine| engine.imaging.clone())
}

pub(crate) fn folder(name: &str) -> PathBuf {
    let folder = std::env::temp_dir().join(format!("rikka-paint-document-{name}"));
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).expect("a folder");
    folder
}

pub(crate) fn write_fixture(name: &str, bytes: &[u8]) -> PathBuf {
    let path = std::env::temp_dir().join(format!("rikka-paint-document-{name}"));
    std::fs::write(&path, bytes).expect("writing the file");
    path
}

pub(crate) fn filled(width: u32, height: u32, rgba: [u8; 4]) -> RgbaImage {
    let pixels: Vec<u8> = (0..width * height).flat_map(|_| rgba).collect();
    RgbaImage::from_rgba8(width, height, &pixels).expect("an image")
}

pub(crate) fn layered() -> rikka_psd::Document {
    let mut document = rikka_psd::Document::new(2, 2, PsdDepth::Eight, ColorMode::Rgb);
    document
        .set_composite_rgba(&filled(2, 2, RED), Compression::Rle)
        .expect("a composite");
    let mut paper = rikka_psd::Layer::new("paper");
    document
        .set_layer_rgba(&mut paper, 0, 0, &filled(2, 2, BLUE), Compression::Rle)
        .expect("a layer");
    let mut ink = rikka_psd::Layer::new("ink");
    document
        .set_layer_rgba(&mut ink, 0, 0, &filled(2, 2, RED), Compression::Rle)
        .expect("a layer");
    document.layers.push(Node::Layer(paper));
    document.layers.push(Node::Group(
        Group::new("drawing").with_children(vec![Node::Layer(ink)]),
    ));
    document.resources.set(
        resource_id::VERSION_INFO,
        ResourceData::VersionInfo(VersionInfo {
            version: 1,
            has_real_merged_data: true,
            writer: String::from("Adobe Photoshop"),
            reader: String::from("Adobe Photoshop CC"),
            file_version: 1,
        }),
    );
    document
}

pub(crate) fn opened_psd(name: &str) -> Document {
    let path = write_fixture(name, &layered().to_bytes().expect("a document"));
    Document::open(&path, &imaging()).expect("an open document")
}

pub(crate) fn opened_picture(name: &str, width: u32, pixels: &[[u8; 4]]) -> Document {
    let height = pixels.len() as u32 / width;
    let png = Image::from_rgba8(width, height, pixels.concat())
        .expect("an image")
        .write(rikka_image::Png)
        .to_bytes()
        .expect("a png");
    Document::open(&write_fixture(name, &png), &imaging()).expect("an open picture")
}

pub(crate) fn rgba8(picture: &rikka_image_edit::Picture) -> Vec<u8> {
    picture
        .to_image()
        .expect("pixels")
        .convert(ColorType::Rgba8)
        .as_bytes()
        .to_vec()
}

pub(crate) fn whole_pixels(document: &Document) -> Vec<u8> {
    rgba8(&document.composite().expect("a composite"))
}

pub(crate) fn first_pixel(document: &Document) -> [u8; 4] {
    pixel_at(document, 0, 0)
}

pub(crate) fn pixel_at(document: &Document, x: u32, y: u32) -> [u8; 4] {
    let at = ((y * document.width() + x) * 4) as usize;
    whole_pixels(document)[at..at + 4]
        .try_into()
        .expect("a pixel")
}

pub(crate) fn bottom(document: &Document) -> LayerId {
    document
        .layers()
        .iter()
        .map(|layer| layer.id)
        .min()
        .expect("a document has a layer")
}

pub(crate) fn edit(document: &mut Document, edit: Edit) -> Change {
    document.apply(&edit).expect("an edit")
}

pub(crate) fn hide(document: &mut Document, layer: LayerId) -> Change {
    edit(
        document,
        Edit::Show {
            layer,
            visible: false,
        },
    )
}

pub(crate) fn develop(document: &mut Document, layer: LayerId, recipe: Recipe) -> Change {
    edit(document, Edit::Develop { layer, recipe })
}

pub(crate) fn select(document: &mut Document, region: Region) -> Change {
    edit(
        document,
        Edit::Select {
            region,
            by: SelectionAction::Rectangle,
        },
    )
}
