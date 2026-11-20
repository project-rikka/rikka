use rikka_mime::MimeType;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Family {
    Folder,
    Image,
    Audio,
    Video,
    Document,
    Text,
    Archive,
    Program,
    Font,
    #[default]
    Other,
}

impl Family {
    #[must_use]
    pub fn from_mime(mime: &MimeType) -> Self {
        match mime.kind() {
            "inode" if mime.is_directory() => Self::Folder,
            "image" => Self::Image,
            "audio" => Self::Audio,
            "video" => Self::Video,
            "font" => Self::Font,
            "text" => match mime.subtype() {
                "html" | "xml" => Self::Document,
                _ => Self::Text,
            },
            "application" => Self::from_application_subtype(mime.subtype()),
            _ => Self::Other,
        }
    }

    fn from_application_subtype(subtype: &str) -> Self {
        const DOCUMENTS: &[&str] = &[
            "pdf",
            "epub+zip",
            "rtf",
            "msword",
            "vnd.ms-excel",
            "vnd.ms-powerpoint",
            "vnd.oasis.opendocument.text",
            "vnd.oasis.opendocument.spreadsheet",
            "vnd.oasis.opendocument.presentation",
            "vnd.oasis.opendocument.graphics",
            "vnd.openxmlformats-officedocument.wordprocessingml.document",
            "vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            "vnd.openxmlformats-officedocument.presentationml.presentation",
        ];
        const ARCHIVES: &[&str] = &[
            "zip",
            "x-7z-compressed",
            "x-tar",
            "x-bzip2",
            "gzip",
            "x-xz",
            "zstd",
            "x-rar",
            "vnd.rar",
            "x-lzma",
            "x-cpio",
            "x-archive",
            "x-iso9660-image",
            "vnd.debian.binary-package",
            "x-rpm",
        ];
        const PROGRAMS: &[&str] = &[
            "x-executable",
            "x-sharedlib",
            "x-pie-executable",
            "x-shellscript",
            "x-desktop",
            "vnd.appimage",
        ];
        const TEXTS: &[&str] = &["json", "xml", "toml", "yaml", "x-yaml", "javascript"];

        if DOCUMENTS.contains(&subtype) {
            Self::Document
        } else if ARCHIVES.contains(&subtype) {
            Self::Archive
        } else if PROGRAMS.contains(&subtype) {
            Self::Program
        } else if TEXTS.contains(&subtype) {
            Self::Text
        } else {
            Self::Other
        }
    }
}
