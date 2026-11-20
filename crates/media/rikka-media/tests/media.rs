use std::path::{Path, PathBuf};

use rikka_audio_decode::AudioFormat;
use rikka_image::{ColorType, Format as Picture, Image};
use rikka_media::{Detail, Error, Kind};

fn picture(directory: &Path, name: &str) -> PathBuf {
    let path = directory.join(name);
    Image::new(30, 20, ColorType::Rgba8)
        .expect("a usable size")
        .save(&path)
        .expect("writes");
    path
}

#[test]
fn format_kind_matches_reader() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let path = picture(directory.path(), "dusk.png");

    assert_eq!(rikka_media_format::kind(&path), Some(Kind::Picture));
    let format = rikka_media_format::identify(&path).expect("a known format");
    assert_eq!(format.name(), "PNG");
    assert_eq!(format.detail(), Detail::Picture(Picture::Png));
}

#[test]
fn content_beats_extension() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let path = directory.path().join("dusk.mp4");
    Image::new(8, 8, ColorType::Rgba8)
        .expect("a usable size")
        .write(Picture::Png)
        .to(&path)
        .expect("writes");

    assert_eq!(rikka_media_format::kind(&path), Some(Kind::Picture));
}

#[test]
fn image_still_and_probe() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let path = picture(directory.path(), "dusk.png");

    let still = rikka_media::still(&path).expect("a picture is its own still");
    assert_eq!(still.dimensions(), (30, 20));

    let info = rikka_media::info(&path).expect("a picture is measured");
    assert_eq!(info.kind(), Kind::Picture);
    assert_eq!(
        info.extent.map(|extent| (extent.width, extent.height)),
        Some((30, 20))
    );
    assert_eq!(info.duration, None);
    assert!(info.bytes > 0);
}

#[test]
fn unknown_media_is_rejected() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let path = directory.path().join("notes.txt");
    std::fs::write(&path, b"not a picture").expect("writes");

    assert_eq!(rikka_media_format::kind(&path), None);
    assert!(rikka_media::still(&path).is_err());
    assert!(matches!(
        rikka_media::info(&path),
        Err(Error::Unsupported(_))
    ));
}

#[test]
fn uncommon_music_formats_are_audio() {
    let named: &[(&str, AudioFormat)] = &[
        ("song.opus", AudioFormat::Opus),
        ("song.wv", AudioFormat::WavPack),
        ("song.ape", AudioFormat::Ape),
        ("song.dsf", AudioFormat::Dsf),
        ("song.dff", AudioFormat::Dff),
        ("song.tak", AudioFormat::Tak),
        ("song.mpc", AudioFormat::Musepack),
        ("song.wma", AudioFormat::Wma),
        ("song.ac3", AudioFormat::Ac3),
        ("song.ec3", AudioFormat::Eac3),
    ];
    for (name, hint) in named {
        let format = rikka_media::Format::from_path(name).expect("a known extension");
        assert_eq!(format.detail(), Detail::Sound(Some(*hint)), "{name}");
    }

    let headers: &[(&[u8], AudioFormat)] = &[
        (b"wvpk", AudioFormat::WavPack),
        (b"MAC ", AudioFormat::Ape),
        (b"tBaK", AudioFormat::Tak),
        (b"MPCK", AudioFormat::Musepack),
        (b"\x0B\x77", AudioFormat::Ac3),
    ];
    for (header, hint) in headers {
        let format = rikka_media::Format::identify("unnamed", header).expect("a known header");
        assert_eq!(format.detail(), Detail::Sound(Some(*hint)), "{header:?}");
    }
}
