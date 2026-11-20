use std::path::PathBuf;

use rikka_image::{ColorScheme, Format, ImageLoader, read};

use crate::support::gradient;

fn photo(width: u32, height: u32) -> (tempfile::TempDir, PathBuf) {
    let directory = tempfile::tempdir().expect("create temp dir");
    let path = directory.path().join("photo.png");
    gradient(width, height).save(&path).expect("writes");
    (directory, path)
}

fn rasterize_svg(svg: &str) -> (tempfile::TempDir, PathBuf) {
    let directory = tempfile::tempdir().expect("create temp dir");
    let path = directory.path().join("logo.svg");
    std::fs::write(&path, svg).expect("writes");
    (directory, path)
}

#[test]
fn loads_files_and_bytes_on_shared_workers() {
    let (_directory, path) = photo(24, 24);
    let loader = ImageLoader::with_threads(2);

    let handles: Vec<_> = (0..16).map(|_| loader.load(&path)).collect();
    for handle in handles {
        assert_eq!(handle.wait().expect("decodes").dimensions(), (24, 24));
    }

    let bytes = gradient(12, 12).encode(Format::Png).expect("encodes");
    assert_eq!(
        loader.load(bytes).wait().expect("decodes").dimensions(),
        (12, 12)
    );
}

#[test]
fn try_take_returns_result_once() {
    let (_directory, path) = photo(16, 16);
    let loader = ImageLoader::with_threads(1);

    let handle = loader.load(&path);
    while !handle.is_ready() {
        std::thread::yield_now();
    }

    let image = handle.try_take().expect("ready").expect("decodes");
    assert_eq!(image.dimensions(), (16, 16));
    assert!(handle.try_take().is_none(), "result is taken once");
}

#[test]
fn thumbnail_request_downscales() {
    let (_directory, path) = photo(64, 32);
    let loader = ImageLoader::new();

    let thumbnail = loader
        .read(&path)
        .thumbnail(16, 16)
        .submit()
        .wait()
        .expect("decodes");

    assert_eq!(thumbnail.dimensions(), (16, 8));
}

#[test]
fn failed_load_reports_error_through_handle() {
    let loader = ImageLoader::new();

    let error = loader
        .load("this-file-does-not-exist.png")
        .wait()
        .expect_err("no such file");

    assert!(matches!(error, rikka_image::Error::Io(_)), "{error:?}");
}

#[test]
fn svg_rasterizes_at_requested_size() {
    let (_directory, path) = rasterize_svg(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 10 10\">\
         <rect width=\"10\" height=\"10\" fill=\"#3366ff\"/></svg>",
    );
    let loader = ImageLoader::new();

    let small = loader.read(&path).vector(24, 24).submit().wait();
    let large = loader.read(&path).vector(96, 96).submit().wait();

    assert_eq!(small.expect("rasterizes").dimensions(), (24, 24));
    assert_eq!(large.expect("rasterizes").dimensions(), (96, 96));
}

#[test]
fn svg_decoded_as_raster_is_rejected() {
    let (_directory, path) =
        rasterize_svg("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"4\" height=\"4\"/>");
    let loader = ImageLoader::new();

    assert!(loader.load(&path).wait().is_err());
}

#[test]
fn svg_rasterizes_at_natural_or_fitted_size() {
    let (_directory, path) = rasterize_svg(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"40\" height=\"20\">\
         <rect width=\"40\" height=\"20\" fill=\"#3366ff\"/></svg>",
    );

    let natural = read(&path).rasterize_svg().expect("rasterizes");
    let fitted = read(&path)
        .rasterize_svg_to_fit(400, 400)
        .expect("rasterizes");

    assert_eq!(natural.dimensions(), (40, 20));
    assert_eq!(fitted.dimensions(), (400, 200));
}

#[test]
fn svg_follows_the_requested_color_scheme() {
    let themed = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"4\" height=\"4\">\
         <style>rect { fill: #ff0000 } @media (prefers-color-scheme: dark) { rect { fill: #0000ff } }</style>\
         <rect width=\"4\" height=\"4\"/></svg>";
    let plain = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"4\" height=\"4\">\
         <rect width=\"4\" height=\"4\" fill=\"#ff0000\"/></svg>";
    let (announce, announced) = std::sync::mpsc::channel();
    let loader = ImageLoader::with_threads(1).notifying(move || {
        let _ = announce.send(());
    });
    let red = [255, 0, 0, 255];
    let blue = [0, 0, 255, 255];
    for (document, scheme, color, follows) in [
        (themed, ColorScheme::Light, red, true),
        (themed, ColorScheme::Dark, blue, true),
        (plain, ColorScheme::Dark, red, false),
    ] {
        let handle = loader
            .read(document.as_bytes().to_vec())
            .vector(4, 4)
            .color_scheme(scheme)
            .submit();
        announced.recv().expect("load is announced");
        let image = handle.try_take().expect("taken").expect("rasterizes");
        assert_eq!(image.rgba8()[..4], color, "{scheme:?}");
        assert_eq!(handle.follows_color_scheme(), follows, "{scheme:?}");
    }
    let fitted = read(themed.as_bytes())
        .color_scheme(ColorScheme::Dark)
        .rasterize_svg()
        .expect("rasterizes");
    assert_eq!(fitted.rgba8()[..4], blue);
}

#[test]
fn finished_load_is_announced() {
    let (_directory, path) = photo(16, 16);
    let (announce, announced) = std::sync::mpsc::channel();
    let loader = ImageLoader::with_threads(1).notifying(move || {
        let _ = announce.send(());
    });

    let handle = loader.load(&path);
    announced.recv().expect("load is announced");
    assert!(handle.is_ready());
    assert_eq!(
        handle
            .try_take()
            .expect("taken")
            .expect("decodes")
            .dimensions(),
        (16, 16)
    );
}
