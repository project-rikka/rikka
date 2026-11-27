use animate_library::{Location, Shelf, ThumbnailSize};
use animate_preferences::Preferences;

#[test]
fn library_preferences_round_trip() {
    let preferences = Preferences::in_memory();
    let locations = [
        Location::shelf("/media/dances", Shelf::Motions),
        Location::library("/media/library"),
    ];

    preferences.keep_library_locations(&locations);
    preferences.keep_library_shelf(Shelf::Cameras);
    preferences.keep_thumbnail_size(ThumbnailSize::Large);

    assert_eq!(preferences.library_locations(), locations);
    assert_eq!(preferences.library_shelf(), Shelf::Cameras);
    assert_eq!(preferences.thumbnail_size(), ThumbnailSize::Large);
}
