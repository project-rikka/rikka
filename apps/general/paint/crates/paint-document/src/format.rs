use std::path::Path;

use rikka_image::Format;

const PHOTOSHOP_MIME: &str = "image/vnd.adobe.photoshop";
const PSD_EXTENSIONS: &[&str] = &["psd"];
const PSB_EXTENSIONS: &[&str] = &["psb"];
const UNWRITTEN: [Format; 2] = [Format::Ico, Format::Dds];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FileFormat {
    Picture(Format),
    Photoshop { large: bool },
}

impl FileFormat {
    #[must_use]
    pub fn writable() -> Vec<FileFormat> {
        [
            FileFormat::Photoshop { large: false },
            FileFormat::Photoshop { large: true },
        ]
        .into_iter()
        .chain(
            Format::ALL
                .into_iter()
                .filter(|format| writes(*format))
                .map(FileFormat::Picture),
        )
        .collect()
    }

    #[must_use]
    pub fn for_path(path: &Path) -> Option<FileFormat> {
        let extension = path.extension()?.to_str()?.to_ascii_lowercase();
        match extension.as_str() {
            "psd" => Some(FileFormat::Photoshop { large: false }),
            "psb" => Some(FileFormat::Photoshop { large: true }),
            _ => Format::from_extension(&extension)
                .filter(|format| writes(*format))
                .map(FileFormat::Picture),
        }
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            FileFormat::Picture(format) => format.name(),
            FileFormat::Photoshop { large: false } => "PSD",
            FileFormat::Photoshop { large: true } => "PSB",
        }
    }

    #[must_use]
    pub fn mime_type(self) -> &'static str {
        match self {
            FileFormat::Picture(format) => format.mime_type(),
            FileFormat::Photoshop { .. } => PHOTOSHOP_MIME,
        }
    }

    #[must_use]
    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            FileFormat::Picture(format) => format.extensions(),
            FileFormat::Photoshop { large: false } => PSD_EXTENSIONS,
            FileFormat::Photoshop { large: true } => PSB_EXTENSIONS,
        }
    }

    #[must_use]
    pub fn takes_quality(self) -> bool {
        matches!(
            self,
            FileFormat::Picture(Format::Jpeg | Format::Avif | Format::JpegXl)
        )
    }

    #[must_use]
    pub fn takes_speed(self) -> bool {
        matches!(self, FileFormat::Picture(Format::Avif | Format::JpegXl))
    }

    #[must_use]
    pub fn takes_compression(self) -> bool {
        self == FileFormat::Picture(Format::Png)
    }

    #[must_use]
    pub fn keeps_alpha(self) -> bool {
        match self {
            FileFormat::Picture(format) => format.supports_alpha(),
            FileFormat::Photoshop { .. } => true,
        }
    }

    pub(crate) fn keeps_float(self) -> bool {
        matches!(
            self,
            FileFormat::Photoshop { .. } | FileFormat::Picture(Format::OpenExr | Format::Hdr)
        )
    }
}

fn writes(format: Format) -> bool {
    format.can_encode() && !UNWRITTEN.contains(&format)
}

#[must_use]
pub fn readable_mime_types() -> Vec<&'static str> {
    let mut types: Vec<&'static str> = Format::ALL
        .into_iter()
        .flat_map(Format::mime_types)
        .copied()
        .collect();
    types.push(PHOTOSHOP_MIME);
    types.sort_unstable();
    types.dedup();
    types
}
