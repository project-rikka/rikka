use std::os::unix::fs::PermissionsExt;

use rikka_image::{ColorType, Image};
use rikka_image_edit::Profile;
use rikka_psd::{ResourceData, resource_id};

use crate::support::*;
use paint_document::{Error, Saved, Saving};

fn saved_psd(path: &std::path::Path) -> rikka_psd::Document {
    rikka_psd::Document::read(&std::fs::read(path).expect("the saved bytes"))
        .expect("a saved document")
}

#[test]
fn sixteen_bit_psd_keeps_depth_and_profile() {
    let pixels: Vec<u16> = [65535, 0, 0, 65535, 0, 0, 65535, 65535].to_vec();
    let png = Image::from_rgba16(2, 1, pixels)
        .expect("an image")
        .with_icc_profile(Some(Profile::display_p3().icc()))
        .write(rikka_image::Png)
        .to_bytes()
        .expect("a png");
    let mut document =
        Document::open(&write_fixture("deep.png", &png), &imaging()).expect("an open picture");
    let path = folder("deep").join("deep.psd");

    document.save(&path, &Saving::default()).expect("a save");

    let saved = saved_psd(&path);
    assert_eq!(saved.depth, rikka_psd::Depth::Sixteen);
    assert_eq!(
        saved.resources.icc_profile(),
        Some(&Profile::display_p3().icc()[..])
    );
}

#[test]
fn psd_save_refreshes_composite_and_thumbnail() {
    let mut document = opened_psd("refresh.psd");
    let drawing = document.layers()[0].id;
    hide(&mut document, drawing);
    let path = folder("refresh").join("refreshed.psd");

    let saved = document.save(&path, &Saving::default()).expect("a save");

    assert_eq!(saved, Saved::Document);
    assert!(!document.is_modified());
    let saved = saved_psd(&path);
    assert!(saved.has_real_composite());
    assert_eq!(saved.composite_rgba8().expect("pixels")[..4], BLUE);
    match saved.resources.get(resource_id::VERSION_INFO) {
        Some(ResourceData::VersionInfo(info)) => assert!(info.has_real_merged_data),
        other => panic!("no version info but {other:?}"),
    }
    let thumbnail = saved.resources.thumbnail().expect("a thumbnail");
    assert_eq!(thumbnail.raw_rgb8().expect("raw pixels")[..3], [0, 0, 255]);
}

#[test]
fn opaque_formats_flatten_onto_white() {
    let mut document = opened_picture("clear.png", 2, &[[0, 0, 0, 0], RED]);
    let path = folder("jpeg").join("flattened.jpg");

    document
        .save(&path, &Saving::new().quality(100))
        .expect("a save");

    let saved = Image::open(&path).expect("the jpeg");
    let pixels = saved.convert(ColorType::Rgb8);
    let white = &pixels.as_bytes()[..3];
    assert!(white.iter().all(|value| *value > 240), "{white:?}");
}

#[test]
fn save_leaves_no_temp_files() {
    let mut document = opened_picture("atomic.png", 2, &[RED, BLUE]);
    let folder = folder("atomic");

    document
        .save(&folder.join("one.webp"), &Saving::default())
        .expect("a save");
    document
        .save(&folder.join("one.webp"), &Saving::default())
        .expect("a second save");
    let unknown = document.save(&folder.join("one.unknown"), &Saving::default());

    assert!(matches!(unknown, Err(Error::Unsupported { .. })));
    let names: Vec<_> = std::fs::read_dir(&folder)
        .expect("the folder")
        .map(|entry| entry.expect("an entry").file_name())
        .collect();
    assert_eq!(names, ["one.webp"]);
}

#[test]
fn overwrite_keeps_permissions_and_symlink() {
    let mut document = opened_picture("overwrite.png", 2, &[RED, BLUE]);
    let folder = folder("overwrite");
    let real = folder.join("real.png");
    let link = folder.join("link.png");
    std::fs::write(&real, b"old").expect("an old file");
    std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o640)).expect("permissions");
    std::os::unix::fs::symlink(&real, &link).expect("a symlink");

    document
        .save(&link, &Saving::default())
        .expect("a save through the link");

    assert!(
        std::fs::symlink_metadata(&link)
            .expect("metadata")
            .file_type()
            .is_symlink()
    );
    assert_ne!(std::fs::read(&real).expect("the file"), b"old");
    let mode = std::fs::metadata(&real)
        .expect("metadata")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o640);
}
