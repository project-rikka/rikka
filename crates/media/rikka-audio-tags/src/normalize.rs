use lofty::picture::{Picture as LoftyPicture, PictureType};
use lofty::tag::{ItemKey, Tag, TagType};

use crate::tags::{Gain, MusicBrainz, Picture, PictureKind, ReplayGain, SortNames, Tags};
use crate::text::Repair;
use crate::values;

struct Source {
    tag: Tag,
    repair: Repair,
}

impl Source {
    fn new(tag: Tag) -> Source {
        let guessed = matches!(
            tag.tag_type(),
            TagType::Id3v1 | TagType::Id3v2 | TagType::RiffInfo | TagType::AiffText
        );
        let repair = if guessed {
            Repair::guess(tag.items().filter_map(|item| item.value().text()))
        } else {
            Repair::Keep
        };
        Source { tag, repair }
    }

    fn texts(&self, key: ItemKey) -> Vec<String> {
        let mut values: Vec<String> = Vec::new();
        for text in self.tag.get_strings(key) {
            let text = self
                .repair
                .apply(text.trim_matches(|c: char| c.is_whitespace() || c == '\0'));
            if !text.is_empty() && !values.iter().any(|existing| *existing == *text) {
                values.push(text.into_owned());
            }
        }
        values
    }
}

struct Sources(Vec<Source>);

impl Sources {
    fn texts(&self, key: ItemKey) -> Vec<String> {
        self.0
            .iter()
            .map(|source| source.texts(key))
            .find(|texts| !texts.is_empty())
            .unwrap_or_default()
    }

    fn text(&self, key: ItemKey) -> Option<String> {
        self.texts(key).into_iter().next()
    }

    fn either(&self, plural: ItemKey, single: ItemKey) -> Vec<String> {
        let plural = self.texts(plural);
        if plural.is_empty() {
            self.texts(single)
        } else {
            plural
        }
    }

    fn gain(&self, r128: ItemKey, gain: ItemKey, peak: ItemKey) -> Option<Gain> {
        if let Some(db) = self.text(r128).as_deref().and_then(values::r128) {
            return Some(Gain { db, peak: None });
        }
        let db = self.text(gain).as_deref().and_then(values::decibels)?;
        Some(Gain {
            db,
            peak: self.text(peak).as_deref().and_then(values::peak),
        })
    }

    fn pictures(&self) -> Vec<Picture> {
        self.0
            .iter()
            .map(|source| source.tag.pictures())
            .find(|pictures| !pictures.is_empty())
            .map(|pictures| pictures.iter().filter_map(picture).collect())
            .unwrap_or_default()
    }
}

pub(crate) fn tags(read: Vec<Tag>) -> Tags {
    let sources = Sources(read.into_iter().map(Source::new).collect());
    let date = |key| sources.text(key).as_deref().and_then(values::date);
    Tags {
        title: sources.text(ItemKey::TrackTitle),
        artists: sources.either(ItemKey::TrackArtists, ItemKey::TrackArtist),
        album: sources.text(ItemKey::AlbumTitle),
        album_artists: sources.either(ItemKey::AlbumArtists, ItemKey::AlbumArtist),
        track: values::position(
            sources.text(ItemKey::TrackNumber).as_deref(),
            sources.text(ItemKey::TrackTotal).as_deref(),
        ),
        disc: values::position(
            sources.text(ItemKey::DiscNumber).as_deref(),
            sources.text(ItemKey::DiscTotal).as_deref(),
        ),
        disc_subtitle: sources.text(ItemKey::SetSubtitle),
        date: date(ItemKey::RecordingDate)
            .or_else(|| date(ItemKey::Year))
            .or_else(|| date(ItemKey::ReleaseDate)),
        original_date: date(ItemKey::OriginalReleaseDate),
        genres: sources.texts(ItemKey::Genre),
        composers: sources.texts(ItemKey::Composer),
        isrc: sources
            .text(ItemKey::Isrc)
            .as_deref()
            .and_then(values::isrc),
        musicbrainz: MusicBrainz {
            recording: sources.text(ItemKey::MusicBrainzRecordingId),
            track: sources.text(ItemKey::MusicBrainzTrackId),
            release: sources.text(ItemKey::MusicBrainzReleaseId),
            release_group: sources.text(ItemKey::MusicBrainzReleaseGroupId),
            artists: sources.texts(ItemKey::MusicBrainzArtistId),
            album_artists: sources.texts(ItemKey::MusicBrainzReleaseArtistId),
        },
        label: sources
            .text(ItemKey::Label)
            .or_else(|| sources.text(ItemKey::Publisher)),
        catalogue_number: sources.text(ItemKey::CatalogNumber),
        compilation: sources
            .text(ItemKey::FlagCompilation)
            .is_some_and(|flag| values::flag(&flag)),
        sort: SortNames {
            artist: sources.text(ItemKey::TrackArtistSortOrder),
            album_artist: sources.text(ItemKey::AlbumArtistSortOrder),
            album: sources.text(ItemKey::AlbumTitleSortOrder),
            title: sources.text(ItemKey::TrackTitleSortOrder),
        },
        gain: ReplayGain {
            track: sources.gain(
                ItemKey::R128TrackGain,
                ItemKey::ReplayGainTrackGain,
                ItemKey::ReplayGainTrackPeak,
            ),
            album: sources.gain(
                ItemKey::R128AlbumGain,
                ItemKey::ReplayGainAlbumGain,
                ItemKey::ReplayGainAlbumPeak,
            ),
        },
        pictures: sources.pictures(),
    }
}

fn picture(picture: &LoftyPicture) -> Option<Picture> {
    if picture.data().is_empty() {
        return None;
    }
    let kind = match picture.pic_type() {
        PictureType::CoverFront => PictureKind::Front,
        PictureType::CoverBack => PictureKind::Back,
        PictureType::Artist | PictureType::LeadArtist | PictureType::Band => PictureKind::Artist,
        PictureType::Other => PictureKind::Unspecified,
        _ => PictureKind::Extra,
    };
    Some(Picture {
        kind,
        mime: picture
            .mime_type()
            .map(|mime| mime.as_str().to_owned())
            .filter(|mime| !mime.is_empty()),
        data: picture.data().to_vec(),
    })
}
