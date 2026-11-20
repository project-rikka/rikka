use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use files_core::{Entry, Location};
use files_thumbs::Thumbnails;
use rikka_file_operations::EntryKind;
use rikka_image::Image;
use rikka_mime::MimeType;
use rikka_thumbnails::{Cache, Size};

struct Sandbox(std::path::PathBuf);

impl Sandbox {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("files-thumbs-e2e-{name}"));
        std::fs::remove_dir_all(&path).ok();
        std::fs::create_dir_all(&path).expect("create directory");
        Self(path)
    }

    fn cache(&self) -> Cache {
        Cache::at(self.0.join("cache"))
    }

    fn picture(&self, name: &str) -> Entry {
        let path = self.0.join(name);
        let pixels: Vec<u8> = (0..600 * 400 * 3)
            .map(|index| u8::try_from(index % 251).unwrap_or(0))
            .collect();
        let image = Image::from_rgb8(600, 400, pixels).expect("build image");
        image.save(&path).expect("save picture");
        self.entry(&path, "image/png")
    }

    fn entry(&self, path: &std::path::Path, essence: &str) -> Entry {
        let metadata = std::fs::metadata(path).expect("file metadata");
        Entry {
            location: Location::from_path(path),
            name: path
                .file_name()
                .expect("file name")
                .to_string_lossy()
                .into_owned(),
            kind: EntryKind::File,
            mime: MimeType::parse(essence).expect("valid media type"),
            size: metadata.len(),
            modified: metadata.modified().ok(),
            created: None,
            mode: 0o644,
            hidden: false,
            link: None,
        }
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

const PATIENCE: Duration = Duration::from_secs(30);

fn thumbnails(cache: Cache) -> (Thumbnails, Receiver<Location>) {
    let (sender, made) = mpsc::channel();
    let sender = Mutex::new(sender);
    let thumbs = Thumbnails::new(
        cache,
        Arc::new(move |at| {
            sender
                .lock()
                .expect("sender lock")
                .send(at)
                .expect("test receiver alive");
        }),
    );
    (thumbs, made)
}

fn wait_for(made: &Receiver<Location>, at: &Location) {
    loop {
        let finished = made
            .recv_timeout(PATIENCE)
            .expect("thumbnail finished in time");
        if finished == *at {
            return;
        }
    }
}

#[test]
fn picture_gets_valid_thumbnail() {
    let sandbox = Sandbox::new("picture");
    let cache = sandbox.cache();
    let entry = sandbox.picture("桜.png");
    let modified = entry.modified.expect("modification time");
    let (thumbs, made) = thumbnails(cache.clone());

    assert!(thumbs.can_thumbnail(&entry), "png is thumbnailable");
    assert!(
        thumbs
            .ready(&entry.location, modified, Size::Large)
            .is_none(),
        "no thumbnail before request"
    );

    thumbs.request(&entry, Size::Large);
    wait_for(&made, &entry.location);

    let thumbnail = thumbs
        .ready(&entry.location, modified, Size::Large)
        .expect("thumbnail");
    assert_eq!(
        thumbnail.path(),
        cache.path_of(entry.location.as_str(), Size::Large)
    );

    let made = rikka_image::read(thumbnail.path())
        .decode()
        .expect("decode thumbnail");
    assert!(made.width() <= Size::Large.pixels());
    assert!(made.height() <= Size::Large.pixels());
    assert!(
        made.width() > 1 && made.height() > 1,
        "not a failure marker"
    );
    assert!(made.width() > made.height());
    assert_eq!(thumbnail.pixels().width, made.width() as f32);
    assert_eq!(thumbnail.pixels().height, made.height() as f32);
}

#[test]
fn drawing_is_stored_light_and_shown_from_its_document() {
    let sandbox = Sandbox::new("drawing");
    let path = sandbox.0.join("themed.svg");
    std::fs::write(
        &path,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"40\" height=\"20\">\
         <style>rect { fill: #ffffff } \
         @media (prefers-color-scheme: dark) { rect { fill: #000000 } }</style>\
         <rect width=\"40\" height=\"20\"/></svg>",
    )
    .expect("write drawing");
    let entry = sandbox.entry(&path, "image/svg+xml");
    let modified = entry.modified.expect("modification time");
    let (thumbs, made) = thumbnails(sandbox.cache());

    thumbs.request(&entry, Size::Large);
    wait_for(&made, &entry.location);

    let thumbnail = thumbs
        .ready(&entry.location, modified, Size::Large)
        .expect("thumbnail");
    assert_eq!(thumbnail.shown(), path);
    let stored = rikka_image::read(thumbnail.path())
        .decode()
        .expect("decode thumbnail");
    assert_eq!(
        stored.dimensions(),
        (Size::Large.pixels(), Size::Large.pixels() / 2)
    );
    assert_eq!(stored.into_rgba8()[..4], [255, 255, 255, 255]);
}

#[test]
fn thumbnail_invalid_after_modification() {
    let sandbox = Sandbox::new("stale");
    let entry = sandbox.picture("photo.png");
    let modified = entry.modified.expect("modification time");
    let (thumbs, made) = thumbnails(sandbox.cache());

    thumbs.request(&entry, Size::Large);
    wait_for(&made, &entry.location);

    let later = modified + Duration::from_secs(60);
    assert!(
        thumbs.ready(&entry.location, later, Size::Large).is_none(),
        "thumbnail stale after modification"
    );
}

#[test]
fn repeated_request_decodes_once() {
    let sandbox = Sandbox::new("twice");
    let entry = sandbox.picture("once.png");
    let later = sandbox.picture("later.png");
    let (thumbs, made) = thumbnails(sandbox.cache());

    thumbs.request(&entry, Size::Large);
    thumbs.request(&entry, Size::Large);
    thumbs.request(&entry, Size::Large);
    assert_eq!(
        made.recv_timeout(PATIENCE).expect("first thumbnail"),
        entry.location
    );
    thumbs.request(&later, Size::Large);

    assert_eq!(
        made.recv_timeout(PATIENCE).expect("second thumbnail"),
        later.location
    );
    assert_eq!(made.try_iter().count(), 0);
}

#[test]
fn unreadable_file_marked_failed() {
    let sandbox = Sandbox::new("broken");
    let path = sandbox.0.join("not-really.png");
    std::fs::write(&path, b"this is not a picture").expect("write file");
    let entry = sandbox.entry(&path, "image/png");
    let modified = entry.modified.expect("modification time");
    let (thumbs, made) = thumbnails(sandbox.cache());

    assert!(!thumbs.known_bad(&entry.location, modified));
    thumbs.request(&entry, Size::Large);
    wait_for(&made, &entry.location);
    assert!(thumbs.known_bad(&entry.location, modified));

    assert!(
        thumbs
            .ready(&entry.location, modified, Size::Large)
            .is_none()
    );
}
