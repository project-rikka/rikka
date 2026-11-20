use crate::tuning::Scale;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Target {
    Picture(PictureFormat),
    Shrink(Scale),
    StripMetadata,
    Sound(SoundFormat),
    Movie(MovieFormat),
    Extract(SoundFormat),
    Model(ModelFormat),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PictureFormat {
    Png,
    Jpeg,
    WebP,
    Avif,
    Gif,
    Tiff,
    Bmp,
    Ico,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SoundFormat {
    Mp3,
    Aac,
    Opus,
    Vorbis,
    Flac,
    Alac,
    Wav,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MovieFormat {
    Mp4,
    WebM,
    Mkv,
    Gif,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ModelFormat {
    Glb,
    Gltf,
    Fbx,
    Pmx,
    Vrm,
}

const PICTURE_SIZES: [Scale; 5] = [
    Scale::Original,
    Scale::LongestSide(3840),
    Scale::LongestSide(1920),
    Scale::LongestSide(1280),
    Scale::LongestSide(640),
];

const MOVIE_SIZES: [Scale; 5] = [
    Scale::Original,
    Scale::ShortSide(2160),
    Scale::ShortSide(1080),
    Scale::ShortSide(720),
    Scale::ShortSide(480),
];

const ANIMATION_SIZES: [Scale; 4] = [
    Scale::LongestSide(480),
    Scale::LongestSide(640),
    Scale::LongestSide(960),
    Scale::Original,
];

impl Target {
    #[must_use]
    pub fn extension(self) -> Option<&'static str> {
        Some(match self {
            Target::Picture(format) => format.extension(),
            Target::Sound(format) | Target::Extract(format) => format.extension(),
            Target::Movie(format) => format.extension(),
            Target::Model(format) => format.extension(),
            Target::Shrink(_) | Target::StripMetadata => return None,
        })
    }

    #[must_use]
    pub fn is_lossy(self) -> bool {
        match self {
            Target::Picture(format) => matches!(
                format,
                PictureFormat::Jpeg | PictureFormat::WebP | PictureFormat::Avif
            ),
            Target::Sound(format) | Target::Extract(format) => format.is_lossy(),
            Target::Movie(format) => matches!(format, MovieFormat::Mp4 | MovieFormat::WebM),
            Target::Shrink(_) | Target::StripMetadata | Target::Model(_) => false,
        }
    }

    #[must_use]
    pub fn sizes(self) -> &'static [Scale] {
        match self {
            Target::Picture(_) => &PICTURE_SIZES,
            Target::Movie(MovieFormat::Mp4 | MovieFormat::WebM) => &MOVIE_SIZES,
            Target::Movie(MovieFormat::Gif) => &ANIMATION_SIZES,
            _ => &[],
        }
    }

    pub(crate) fn matches_format(self, name: &str) -> bool {
        let extension = extension_of(name);
        let is = |known: &str| extension.eq_ignore_ascii_case(known);
        match self {
            Target::Picture(format) => {
                rikka_image::Format::from_extension(extension) == Some(format.image())
            }
            Target::Sound(format) => match format {
                SoundFormat::Mp3 => is("mp3"),
                SoundFormat::Opus => is("opus"),
                SoundFormat::Vorbis => is("ogg") || is("oga"),
                SoundFormat::Flac => is("flac"),
                SoundFormat::Wav => is("wav") || is("wave"),
                SoundFormat::Aac | SoundFormat::Alac => false,
            },
            Target::Movie(format) => {
                is(format.extension()) || (format == MovieFormat::Mp4 && is("m4v"))
            }
            Target::Model(format) => format != ModelFormat::Vrm && is(format.extension()),
            Target::Shrink(_) | Target::StripMetadata | Target::Extract(_) => false,
        }
    }
}

pub(crate) fn extension_of(name: &str) -> &str {
    name.rsplit_once('.')
        .filter(|(stem, _)| !stem.is_empty())
        .map_or("", |(_, extension)| extension)
}

impl PictureFormat {
    pub const ALL: [PictureFormat; 8] = [
        PictureFormat::Png,
        PictureFormat::Jpeg,
        PictureFormat::WebP,
        PictureFormat::Avif,
        PictureFormat::Gif,
        PictureFormat::Tiff,
        PictureFormat::Bmp,
        PictureFormat::Ico,
    ];

    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            PictureFormat::Png => "png",
            PictureFormat::Jpeg => "jpg",
            PictureFormat::WebP => "webp",
            PictureFormat::Avif => "avif",
            PictureFormat::Gif => "gif",
            PictureFormat::Tiff => "tiff",
            PictureFormat::Bmp => "bmp",
            PictureFormat::Ico => "ico",
        }
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        self.image().name()
    }

    pub(crate) const fn image(self) -> rikka_image::Format {
        match self {
            PictureFormat::Png => rikka_image::Format::Png,
            PictureFormat::Jpeg => rikka_image::Format::Jpeg,
            PictureFormat::WebP => rikka_image::Format::WebP,
            PictureFormat::Avif => rikka_image::Format::Avif,
            PictureFormat::Gif => rikka_image::Format::Gif,
            PictureFormat::Tiff => rikka_image::Format::Tiff,
            PictureFormat::Bmp => rikka_image::Format::Bmp,
            PictureFormat::Ico => rikka_image::Format::Ico,
        }
    }
}

impl SoundFormat {
    pub const ALL: [SoundFormat; 7] = [
        SoundFormat::Mp3,
        SoundFormat::Aac,
        SoundFormat::Opus,
        SoundFormat::Vorbis,
        SoundFormat::Flac,
        SoundFormat::Alac,
        SoundFormat::Wav,
    ];

    pub const AUDIO_EXTRACTION: [SoundFormat; 5] = [
        SoundFormat::Mp3,
        SoundFormat::Aac,
        SoundFormat::Opus,
        SoundFormat::Flac,
        SoundFormat::Wav,
    ];

    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            SoundFormat::Mp3 => "mp3",
            SoundFormat::Aac | SoundFormat::Alac => "m4a",
            SoundFormat::Opus => "opus",
            SoundFormat::Vorbis => "ogg",
            SoundFormat::Flac => "flac",
            SoundFormat::Wav => "wav",
        }
    }

    #[must_use]
    pub const fn is_lossy(self) -> bool {
        matches!(
            self,
            SoundFormat::Mp3 | SoundFormat::Aac | SoundFormat::Opus | SoundFormat::Vorbis
        )
    }
}

impl MovieFormat {
    pub const ALL: [MovieFormat; 4] = [
        MovieFormat::Mp4,
        MovieFormat::WebM,
        MovieFormat::Mkv,
        MovieFormat::Gif,
    ];

    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            MovieFormat::Mp4 => "mp4",
            MovieFormat::WebM => "webm",
            MovieFormat::Mkv => "mkv",
            MovieFormat::Gif => "gif",
        }
    }
}

impl ModelFormat {
    pub const ALL: [ModelFormat; 5] = [
        ModelFormat::Glb,
        ModelFormat::Gltf,
        ModelFormat::Fbx,
        ModelFormat::Pmx,
        ModelFormat::Vrm,
    ];

    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            ModelFormat::Glb => "glb",
            ModelFormat::Gltf => "gltf",
            ModelFormat::Fbx => "fbx",
            ModelFormat::Pmx => "pmx",
            ModelFormat::Vrm => "vrm",
        }
    }
}
