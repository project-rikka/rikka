use proton_files::{Entry, Thumb, Thumbs, preview};
use rikka_image::{Format, Image};
use rikka_proton_drive::LinkId;

const WIDTH: u32 = 640;
const HEIGHT: u32 = 320;

fn entry(name: &str, pictured: bool) -> Entry {
    Entry {
        id: LinkId::from(name),
        name: name.to_owned(),
        folder: false,
        size: 1024,
        media_type: "image/png".to_owned(),
        modified: jiff::Timestamp::from_second(42).unwrap(),
        trashed: false,
        pictured,
    }
}

fn picture() -> Image {
    let pixels: Vec<u8> = (0..WIDTH * HEIGHT)
        .flat_map(|at| {
            let shade = (at % 251) as u8;
            [shade, 255 - shade, shade / 2, 255]
        })
        .collect();

    Image::from_rgba8(WIDTH, HEIGHT, pixels).expect("a picture is made")
}

fn thumb() -> Thumb {
    let encoded = picture().encode(Format::Png).expect("the picture encodes");

    Thumb::decode(encoded).expect("the thumbnail is read")
}

#[test]
fn drops_thumbnail_of_stale_revision() {
    let mut thumbs = Thumbs::new();
    let asking = thumbs.request([entry("photo.png", true)].iter());
    let newer = [Entry {
        modified: jiff::Timestamp::from_second(43).unwrap(),
        ..entry("photo.png", true)
    }];

    drop(thumbs.request(newer.iter()));

    assert!(!thumbs.insert(&asking[0], Some(thumb())));
    assert!(thumbs.all().is_empty());
}

#[test]
fn preview_encodes_local_picture() {
    let folder = tempfile::tempdir().expect("a folder");
    let path = folder.path().join("holiday.png");
    picture().save(&path).expect("the picture is written");

    let made = preview(&path).expect("a preview is made");

    assert!(made.len() <= 60 * 1024, "{} bytes", made.len());
    assert_eq!(
        rikka_image::read(made.as_slice())
            .probe()
            .expect("the preview is a picture")
            .format,
        Format::Jpeg
    );
}
