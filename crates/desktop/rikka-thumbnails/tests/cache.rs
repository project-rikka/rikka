use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

use rikka_thumbnails::{Cache, Size};

const URI: &str = "file:///tmp/a%20b.png";

#[test]
fn thumbnail_path_follows_spec() {
    let cache = Cache::at("/cache/thumbnails");

    assert_eq!(
        cache.path_of(URI, Size::Normal),
        Path::new("/cache/thumbnails/normal/f2584ab78dd95a88bd0d3f0ecaee7a8c.png")
    );
    assert!(
        cache
            .path_of(URI, Size::XxLarge)
            .starts_with("/cache/thumbnails/xx-large")
    );
    assert!(
        cache
            .failure_path(URI, "rikka-files")
            .starts_with("/cache/thumbnails/fail/rikka-files")
    );
}

#[test]
fn fields_follow_spec() {
    let fields = Cache::fields(URI, UNIX_EPOCH + Duration::from_secs(90), Some(7));

    assert!(fields.contains(&("Thumb::URI".to_owned(), URI.to_owned())));
    assert!(fields.contains(&("Thumb::MTime".to_owned(), "90".to_owned())));
    assert!(fields.contains(&("Thumb::Size".to_owned(), "7".to_owned())));
}
