use lofty::tag::{ItemKey, TagType};

use crate::support;

fn as_latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| char::from(*byte)).collect()
}

#[test]
fn id3v1_legacy_encoding_is_detected() {
    let shift_jis = encoding_bytes("宇多田ヒカル", encoding_rs::SHIFT_JIS);
    let shift_jis_album = encoding_bytes("初恋の歌", encoding_rs::SHIFT_JIS);
    let cases = [
        (
            "shift_jis",
            vec![as_latin1(&shift_jis), as_latin1(&shift_jis_album)],
            ["宇多田ヒカル", "初恋の歌"],
        ),
        (
            "utf-8 stored as latin-1",
            vec![
                as_latin1("Beyoncé".as_bytes()),
                as_latin1("Café".as_bytes()),
            ],
            ["Beyoncé", "Café"],
        ),
        (
            "real latin-1",
            vec!["Sigur Rós".to_owned(), "Ágætis byrjun".to_owned()],
            ["Sigur Rós", "Ágætis byrjun"],
        ),
    ];
    let dir = support::folder();
    for (index, (name, raw, expected)) in cases.into_iter().enumerate() {
        let path = support::mp3(dir.path(), &format!("{index}.mp3"), 40);
        let tag = support::tag(
            TagType::Id3v1,
            &[
                (ItemKey::TrackArtist, &raw[0]),
                (ItemKey::AlbumTitle, &raw[1]),
            ],
        );
        support::save(&path, &tag);
        let tags = music_tags::read(&path)
            .unwrap_or_else(|error| panic!("{name}: {error}"))
            .tags;
        assert_eq!(tags.artists, [expected[0]], "{name}");
        assert_eq!(tags.album.as_deref(), Some(expected[1]), "{name}");
    }
}

fn encoding_bytes(text: &str, encoding: &'static encoding_rs::Encoding) -> Vec<u8> {
    encoding.encode(text).0.into_owned()
}
