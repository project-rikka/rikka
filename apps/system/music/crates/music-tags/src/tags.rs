#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tags {
    pub title: Option<String>,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub album_artists: Vec<String>,
    pub track: Position,
    pub disc: Position,
    pub disc_subtitle: Option<String>,
    pub date: Option<Date>,
    pub original_date: Option<Date>,
    pub genres: Vec<String>,
    pub composers: Vec<String>,
    pub isrc: Option<String>,
    pub musicbrainz: MusicBrainz,
    pub label: Option<String>,
    pub catalogue_number: Option<String>,
    pub compilation: bool,
    pub sort: SortNames,
    pub gain: ReplayGain,
    pub pictures: Vec<Picture>,
}

impl Tags {
    #[must_use]
    pub fn cover(&self) -> Option<&Picture> {
        [PictureKind::Front, PictureKind::Unspecified]
            .into_iter()
            .find_map(|kind| self.pictures.iter().find(|picture| picture.kind == kind))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Position {
    pub number: Option<u32>,
    pub total: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    pub year: u16,
    pub month: Option<u8>,
    pub day: Option<u8>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MusicBrainz {
    pub recording: Option<String>,
    pub track: Option<String>,
    pub release: Option<String>,
    pub release_group: Option<String>,
    pub artists: Vec<String>,
    pub album_artists: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SortNames {
    pub artist: Option<String>,
    pub album_artist: Option<String>,
    pub album: Option<String>,
    pub title: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ReplayGain {
    pub track: Option<Gain>,
    pub album: Option<Gain>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gain {
    pub db: f32,
    pub peak: Option<f32>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Picture {
    pub kind: PictureKind,
    pub mime: Option<String>,
    pub data: Vec<u8>,
}

impl std::fmt::Debug for Picture {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Picture")
            .field("kind", &self.kind)
            .field("mime", &self.mime)
            .field("bytes", &self.data.len())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PictureKind {
    Front,
    Back,
    Artist,
    Unspecified,
    Extra,
}
