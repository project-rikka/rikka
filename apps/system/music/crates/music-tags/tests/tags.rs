use lofty::picture::PictureType;
use lofty::tag::{ItemKey, TagType};
use music_tags::{Date, Gain, PictureKind, Position};

use crate::support;

#[test]
fn vorbis_comments_keep_all_values() {
    let dir = support::folder();
    let path = support::flac(dir.path(), "a.flac", 44_100, 16, 44_100);
    let mut tag = support::tag(
        TagType::VorbisComments,
        &[
            (ItemKey::TrackTitle, "Bridge Over Troubled Water"),
            (ItemKey::TrackArtist, "Simon & Garfunkel"),
            (ItemKey::TrackArtist, "Guest, Featured"),
            (ItemKey::AlbumTitle, "Bridge"),
            (ItemKey::AlbumArtist, "Simon & Garfunkel"),
            (ItemKey::TrackNumber, "3"),
            (ItemKey::TrackTotal, "11"),
            (ItemKey::DiscNumber, "2"),
            (ItemKey::DiscTotal, "2"),
            (ItemKey::SetSubtitle, "Bonus"),
            (ItemKey::RecordingDate, "1970-01-26"),
            (ItemKey::OriginalReleaseDate, "1969"),
            (ItemKey::Genre, "Folk"),
            (ItemKey::Genre, "Rock; Pop"),
            (ItemKey::Composer, "Paul Simon"),
            (ItemKey::Isrc, "us-sm1-70-00001"),
            (ItemKey::MusicBrainzRecordingId, "rec"),
            (ItemKey::MusicBrainzTrackId, "trk"),
            (ItemKey::MusicBrainzReleaseId, "rel"),
            (ItemKey::MusicBrainzReleaseGroupId, "grp"),
            (ItemKey::MusicBrainzArtistId, "art-1"),
            (ItemKey::MusicBrainzArtistId, "art-2"),
            (ItemKey::MusicBrainzReleaseArtistId, "alb-art"),
            (ItemKey::Label, "Columbia"),
            (ItemKey::CatalogNumber, "KCS 9914"),
            (ItemKey::FlagCompilation, "1"),
            (ItemKey::TrackArtistSortOrder, "Simon and Garfunkel"),
            (ItemKey::AlbumTitleSortOrder, "Bridge, The"),
            (ItemKey::ReplayGainTrackGain, "-6.54 dB"),
            (ItemKey::ReplayGainTrackPeak, "0.988"),
            (ItemKey::ReplayGainAlbumGain, "+1,25 dB"),
        ],
    );
    tag.push_picture(support::picture(PictureType::CoverBack, b"back"));
    tag.push_picture(support::picture(PictureType::CoverFront, b"front"));
    support::save(&path, &tag);

    let tags = music_tags::read(&path).unwrap().tags;
    assert_eq!(tags.title.as_deref(), Some("Bridge Over Troubled Water"));
    assert_eq!(tags.artists, ["Simon & Garfunkel", "Guest, Featured"]);
    assert_eq!(tags.album_artists, ["Simon & Garfunkel"]);
    assert_eq!(
        tags.track,
        Position {
            number: Some(3),
            total: Some(11)
        }
    );
    assert_eq!(
        tags.disc,
        Position {
            number: Some(2),
            total: Some(2)
        }
    );
    assert_eq!(tags.disc_subtitle.as_deref(), Some("Bonus"));
    assert_eq!(
        tags.date,
        Some(Date {
            year: 1970,
            month: Some(1),
            day: Some(26)
        })
    );
    assert_eq!(tags.original_date.map(|date| date.year), Some(1969));
    assert_eq!(tags.genres, ["Folk", "Rock; Pop"]);
    assert_eq!(tags.composers, ["Paul Simon"]);
    assert_eq!(tags.isrc.as_deref(), Some("USSM17000001"));
    assert_eq!(tags.musicbrainz.recording.as_deref(), Some("rec"));
    assert_eq!(tags.musicbrainz.track.as_deref(), Some("trk"));
    assert_eq!(tags.musicbrainz.release.as_deref(), Some("rel"));
    assert_eq!(tags.musicbrainz.release_group.as_deref(), Some("grp"));
    assert_eq!(tags.musicbrainz.artists, ["art-1", "art-2"]);
    assert_eq!(tags.musicbrainz.album_artists, ["alb-art"]);
    assert_eq!(tags.label.as_deref(), Some("Columbia"));
    assert_eq!(tags.catalogue_number.as_deref(), Some("KCS 9914"));
    assert!(tags.compilation);
    assert_eq!(tags.sort.artist.as_deref(), Some("Simon and Garfunkel"));
    assert_eq!(tags.sort.album.as_deref(), Some("Bridge, The"));
    assert_eq!(
        tags.gain.track,
        Some(Gain {
            db: -6.54,
            peak: Some(0.988)
        })
    );
    assert_eq!(
        tags.gain.album,
        Some(Gain {
            db: 1.25,
            peak: None
        })
    );
    assert_eq!(tags.pictures.len(), 2);
    let cover = tags.cover().unwrap();
    assert_eq!(cover.kind, PictureKind::Front);
    assert_eq!(cover.data, b"front");
}

#[test]
fn id3v2_splits_multi_value_frames() {
    let dir = support::folder();
    let path = support::mp3(dir.path(), "a.mp3", 40);
    let mut tag = support::tag(
        TagType::Id3v2,
        &[
            (ItemKey::TrackTitle, "Thunderstruck"),
            (ItemKey::TrackArtist, "AC/DC"),
            (ItemKey::TrackArtist, "Guest"),
            (ItemKey::AlbumArtist, "AC/DC"),
            (ItemKey::TrackNumber, "5"),
            (ItemKey::TrackTotal, "9"),
            (ItemKey::RecordingDate, "1990"),
            (ItemKey::Genre, "Hard Rock"),
            (ItemKey::FlagCompilation, "1"),
            (ItemKey::MusicBrainzReleaseId, "release"),
            (ItemKey::ReplayGainTrackGain, "-8.10 dB"),
            (ItemKey::ReplayGainTrackPeak, "1.02"),
        ],
    );
    tag.push_picture(support::picture(PictureType::Other, b"unspecified"));
    support::save(&path, &tag);

    let tags = music_tags::read(&path).unwrap().tags;
    assert_eq!(tags.artists, ["AC/DC", "Guest"]);
    assert_eq!(tags.album_artists, ["AC/DC"]);
    assert_eq!(
        tags.track,
        Position {
            number: Some(5),
            total: Some(9)
        }
    );
    assert_eq!(tags.date.map(|date| date.year), Some(1990));
    assert_eq!(tags.genres, ["Hard Rock"]);
    assert!(tags.compilation);
    assert_eq!(tags.musicbrainz.release.as_deref(), Some("release"));
    assert_eq!(
        tags.gain.track,
        Some(Gain {
            db: -8.1,
            peak: Some(1.02)
        })
    );
    assert_eq!(
        tags.cover().map(|cover| cover.kind),
        Some(PictureKind::Unspecified)
    );
}

#[test]
fn replay_gain_read_from_all_tag_families() {
    let dir = support::folder();
    let cases = [
        (
            "ape on mp3",
            support::mp3(dir.path(), "ape.mp3", 40),
            support::tag(
                TagType::Ape,
                &[
                    (ItemKey::TrackTitle, "Ape"),
                    (ItemKey::ReplayGainTrackGain, "-2.00 dB"),
                    (ItemKey::ReplayGainAlbumGain, "-3.50 dB"),
                    (ItemKey::ReplayGainAlbumPeak, "0.5"),
                ],
            ),
            Some(Gain {
                db: -2.0,
                peak: None,
            }),
            Some(Gain {
                db: -3.5,
                peak: Some(0.5),
            }),
        ),
        (
            "mp4 freeform",
            support::mp4(dir.path(), "a.m4a", support::Sample::Aac),
            support::tag(
                TagType::Mp4Ilst,
                &[
                    (ItemKey::TrackTitle, "Mp4"),
                    (ItemKey::ReplayGainTrackGain, "-4.25 dB"),
                    (ItemKey::ReplayGainTrackPeak, "0.75"),
                ],
            ),
            Some(Gain {
                db: -4.25,
                peak: Some(0.75),
            }),
            None,
        ),
        (
            "id3v2 in wav",
            support::wav(
                dir.path(),
                "a.wav",
                44_100,
                rikka_audio_decode::WavDepth::Pcm16,
                0.1,
            ),
            support::tag(
                TagType::Id3v2,
                &[
                    (ItemKey::TrackTitle, "Wave"),
                    (ItemKey::ReplayGainAlbumGain, "0.5 dB"),
                ],
            ),
            None,
            Some(Gain {
                db: 0.5,
                peak: None,
            }),
        ),
        (
            "opus r128 converted to the replay gain reference",
            support::opus(dir.path(), "a.opus", 1),
            support::tag(
                TagType::VorbisComments,
                &[
                    (ItemKey::TrackTitle, "Opus"),
                    (ItemKey::R128TrackGain, "-1280"),
                    (ItemKey::R128AlbumGain, "256"),
                    (ItemKey::ReplayGainTrackGain, "-9.00 dB"),
                ],
            ),
            Some(Gain {
                db: 0.0,
                peak: None,
            }),
            Some(Gain {
                db: 6.0,
                peak: None,
            }),
        ),
    ];
    for (name, path, tag, track, album) in cases {
        support::save(&path, &tag);
        let tags = music_tags::read(&path)
            .unwrap_or_else(|error| panic!("{name}: {error}"))
            .tags;
        assert!(tags.title.is_some(), "{name}");
        assert_eq!(tags.gain.track, track, "{name}");
        assert_eq!(tags.gain.album, album, "{name}");
    }
}

#[test]
fn mp4_freeform_names_are_case_insensitive() {
    use lofty::config::WriteOptions;
    use lofty::mp4::{Atom, AtomData, AtomIdent, Ilst};
    use lofty::tag::TagExt;
    use std::borrow::Cow;

    let dir = support::folder();
    let path = support::mp4(dir.path(), "a.m4a", support::Sample::Aac);
    let mut ilst = Ilst::new();
    for (name, value) in [
        ("REPLAYGAIN_TRACK_GAIN", "-3.20 dB"),
        ("MusicBrainz Album Id", "release-id"),
    ] {
        ilst.insert(Atom::new(
            AtomIdent::Freeform {
                mean: Cow::Borrowed("com.apple.iTunes"),
                name: Cow::Owned(name.to_owned()),
            },
            AtomData::UTF8(value.to_owned()),
        ));
    }
    ilst.save_to_path(&path, WriteOptions::default()).unwrap();

    let tags = music_tags::read(&path).unwrap().tags;
    assert_eq!(
        tags.gain.track,
        Some(Gain {
            db: -3.2,
            peak: None
        })
    );
    assert_eq!(tags.musicbrainz.release.as_deref(), Some("release-id"));
}
