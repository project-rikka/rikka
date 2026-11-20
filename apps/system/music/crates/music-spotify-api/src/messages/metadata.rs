#[derive(Clone, PartialEq, prost::Message)]
pub struct Track {
    #[prost(bytes = "vec", tag = "1")]
    pub gid: Vec<u8>,
    #[prost(string, tag = "2")]
    pub name: String,
    #[prost(message, optional, tag = "3")]
    pub album: Option<Album>,
    #[prost(message, repeated, tag = "4")]
    pub artist: Vec<Artist>,
    #[prost(sint32, tag = "5")]
    pub number: i32,
    #[prost(sint32, tag = "7")]
    pub duration: i32,
    #[prost(bool, tag = "9")]
    pub explicit: bool,
    #[prost(message, repeated, tag = "10")]
    pub external_id: Vec<ExternalId>,
    #[prost(message, repeated, tag = "11")]
    pub restriction: Vec<Restriction>,
    #[prost(message, repeated, tag = "12")]
    pub file: Vec<AudioFile>,
    #[prost(message, repeated, tag = "13")]
    pub alternative: Vec<Track>,
    #[prost(int64, tag = "17")]
    pub earliest_live_timestamp: i64,
    #[prost(message, repeated, tag = "19")]
    pub availability: Vec<Availability>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct Album {
    #[prost(bytes = "vec", tag = "1")]
    pub gid: Vec<u8>,
    #[prost(string, tag = "2")]
    pub name: String,
    #[prost(message, repeated, tag = "3")]
    pub artist: Vec<Artist>,
    #[prost(enumeration = "AlbumType", optional, tag = "4")]
    pub album_type: Option<i32>,
    #[prost(message, optional, tag = "6")]
    pub date: Option<Date>,
    #[prost(message, repeated, tag = "9")]
    pub cover: Vec<Image>,
    #[prost(message, repeated, tag = "11")]
    pub disc: Vec<Disc>,
    #[prost(message, optional, tag = "17")]
    pub cover_group: Option<ImageGroup>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct Artist {
    #[prost(bytes = "vec", tag = "1")]
    pub gid: Vec<u8>,
    #[prost(string, tag = "2")]
    pub name: String,
    #[prost(message, repeated, tag = "4")]
    pub top_track: Vec<TopTracks>,
    #[prost(message, repeated, tag = "5")]
    pub album_group: Vec<AlbumGroup>,
    #[prost(message, repeated, tag = "6")]
    pub single_group: Vec<AlbumGroup>,
    #[prost(message, repeated, tag = "11")]
    pub portrait: Vec<Image>,
    #[prost(message, optional, tag = "17")]
    pub portrait_group: Option<ImageGroup>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct TopTracks {
    #[prost(string, tag = "1")]
    pub country: String,
    #[prost(message, repeated, tag = "2")]
    pub track: Vec<Track>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct AlbumGroup {
    #[prost(message, repeated, tag = "1")]
    pub album: Vec<Album>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct Date {
    #[prost(sint32, tag = "1")]
    pub year: i32,
    #[prost(sint32, optional, tag = "2")]
    pub month: Option<i32>,
    #[prost(sint32, optional, tag = "3")]
    pub day: Option<i32>,
    #[prost(sint32, tag = "4")]
    pub hour: i32,
    #[prost(sint32, tag = "5")]
    pub minute: i32,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct Image {
    #[prost(bytes = "vec", tag = "1")]
    pub file_id: Vec<u8>,
    #[prost(enumeration = "ImageSize", tag = "2")]
    pub size: i32,
    #[prost(sint32, tag = "3")]
    pub width: i32,
    #[prost(sint32, tag = "4")]
    pub height: i32,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ImageGroup {
    #[prost(message, repeated, tag = "1")]
    pub image: Vec<Image>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct Disc {
    #[prost(sint32, tag = "1")]
    pub number: i32,
    #[prost(message, repeated, tag = "3")]
    pub track: Vec<Track>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct Restriction {
    #[prost(string, optional, tag = "2")]
    pub countries_allowed: Option<String>,
    #[prost(string, optional, tag = "3")]
    pub countries_forbidden: Option<String>,
    #[prost(string, repeated, tag = "5")]
    pub catalogue_str: Vec<String>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct Availability {
    #[prost(string, repeated, tag = "1")]
    pub catalogue_str: Vec<String>,
    #[prost(message, optional, tag = "2")]
    pub start: Option<Date>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ExternalId {
    #[prost(string, tag = "1")]
    pub kind: String,
    #[prost(string, tag = "2")]
    pub id: String,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct AudioFile {
    #[prost(bytes = "vec", tag = "1")]
    pub file_id: Vec<u8>,
    #[prost(enumeration = "AudioFormat", optional, tag = "2")]
    pub format: Option<i32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration)]
#[repr(i32)]
pub enum AlbumType {
    Album = 1,
    Single = 2,
    Compilation = 3,
    Ep = 4,
    Audiobook = 5,
    Podcast = 6,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration)]
#[repr(i32)]
pub enum ImageSize {
    Default = 0,
    Small = 1,
    Large = 2,
    Xlarge = 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration)]
#[repr(i32)]
pub enum AudioFormat {
    OggVorbis96 = 0,
    OggVorbis160 = 1,
    OggVorbis320 = 2,
    Mp3256 = 3,
    Mp3320 = 4,
    Mp3160 = 5,
    Mp396 = 6,
    Mp3160Enc = 7,
    Aac24 = 8,
    Aac48 = 9,
    Flac = 16,
    XheAac24 = 18,
    XheAac16 = 19,
    XheAac12 = 20,
    Flac24Bit = 22,
}
