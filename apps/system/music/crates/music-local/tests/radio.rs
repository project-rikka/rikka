use std::collections::HashSet;

use lofty::tag::ItemKey;

use crate::support::{CD, Fixture, block, tagged};

fn song(fixture: &Fixture, path: &str, title: &str, artist: &str, genre: &str, year: &str) {
    let mut items = tagged(title, artist, path);
    items.extend([(ItemKey::Genre, genre), (ItemKey::RecordingDate, year)]);
    fixture.song(&format!("{path}.flac"), CD, &items);
}

#[test]
fn radio_starts_with_seed() {
    let fixture = Fixture::new();
    song(&fixture, "a/seed", "Seed", "Alpha", "Rock", "2000");
    song(&fixture, "a/later", "Later", "Alpha", "Pop", "2015");
    song(&fixture, "b/peer", "Peer", "Beta", "Rock", "2002");
    song(&fixture, "c/stranger", "Stranger", "Gamma", "Jazz", "1960");
    let fixture_library = fixture.open();
    let catalog = &fixture_library.local.catalog;
    let seed = block(catalog.search("seed")).unwrap().tracks[0].clone();

    let mixed = block(catalog.mix(&seed)).unwrap();

    assert!(mixed[0].is(&seed));
    let names: HashSet<&str> = mixed[1..].iter().map(|track| track.name.as_str()).collect();
    assert_eq!(names, HashSet::from(["Later", "Peer"]));
}
