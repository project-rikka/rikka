use std::fmt;
use std::path::Path;

use crate::stream::StreamCompression;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Format {
    Zip,
    SevenZ,
    Tar,
    TarGzip,
    TarBzip2,
    TarXz,
    TarZstd,
    Gzip,
    Bzip2,
    Xz,
    Zstd,
}

impl Format {
    pub const ALL: [Format; 11] = [
        Format::Zip,
        Format::SevenZ,
        Format::TarGzip,
        Format::TarBzip2,
        Format::TarXz,
        Format::TarZstd,
        Format::Tar,
        Format::Gzip,
        Format::Bzip2,
        Format::Xz,
        Format::Zstd,
    ];

    #[must_use]
    pub const fn is_container(self) -> bool {
        matches!(
            self,
            Format::Zip
                | Format::SevenZ
                | Format::Tar
                | Format::TarGzip
                | Format::TarBzip2
                | Format::TarXz
                | Format::TarZstd
        )
    }

    #[must_use]
    pub const fn supports_encryption(self) -> bool {
        matches!(self, Format::Zip | Format::SevenZ)
    }

    #[must_use]
    pub const fn has_per_entry_compression(self) -> bool {
        matches!(self, Format::Zip | Format::SevenZ)
    }

    #[must_use]
    pub const fn stream_compression(self) -> Option<StreamCompression> {
        match self {
            Format::TarGzip | Format::Gzip => Some(StreamCompression::Gzip),
            Format::TarBzip2 | Format::Bzip2 => Some(StreamCompression::Bzip2),
            Format::TarXz | Format::Xz => Some(StreamCompression::Xz),
            Format::TarZstd | Format::Zstd => Some(StreamCompression::Zstd),
            _ => None,
        }
    }

    #[must_use]
    pub(crate) const fn into_tar(self) -> Format {
        match self {
            Format::Gzip => Format::TarGzip,
            Format::Bzip2 => Format::TarBzip2,
            Format::Xz => Format::TarXz,
            Format::Zstd => Format::TarZstd,
            other => other,
        }
    }

    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Format::Zip => "zip",
            Format::SevenZ => "7z",
            Format::Tar => "tar",
            Format::TarGzip => "tar.gz",
            Format::TarBzip2 => "tar.bz2",
            Format::TarXz => "tar.xz",
            Format::TarZstd => "tar.zst",
            Format::Gzip => "gz",
            Format::Bzip2 => "bz2",
            Format::Xz => "xz",
            Format::Zstd => "zst",
        }
    }

    #[must_use]
    pub fn from_path(path: impl AsRef<Path>) -> Option<Format> {
        let name = path.as_ref().file_name()?.to_str()?;
        suffix_of(name).map(|(_, format)| format)
    }

    #[must_use]
    pub fn split(name: &str) -> Option<(&str, Format)> {
        let (suffix, format) = suffix_of(name).filter(|(suffix, _)| name.len() > suffix.len())?;
        Some((&name[..name.len() - suffix.len()], format))
    }

    #[must_use]
    pub(crate) fn strip_extension(self, file_name: &str) -> String {
        let lower = file_name.to_ascii_lowercase();
        let suffix = format!(".{}", self.extension());
        if lower.ends_with(&suffix) {
            return file_name[..file_name.len() - suffix.len()].to_owned();
        }
        file_name.to_owned()
    }
}

const SUFFIXES: [(&str, Format); 20] = [
    (".tar.gz", Format::TarGzip),
    (".tar.bz2", Format::TarBzip2),
    (".tar.xz", Format::TarXz),
    (".tar.zst", Format::TarZstd),
    (".tar.zstd", Format::TarZstd),
    (".tgz", Format::TarGzip),
    (".tbz", Format::TarBzip2),
    (".tbz2", Format::TarBzip2),
    (".txz", Format::TarXz),
    (".tzst", Format::TarZstd),
    (".zip", Format::Zip),
    (".zipx", Format::Zip),
    (".jar", Format::Zip),
    (".7z", Format::SevenZ),
    (".tar", Format::Tar),
    (".gz", Format::Gzip),
    (".bz2", Format::Bzip2),
    (".xz", Format::Xz),
    (".zst", Format::Zstd),
    (".zstd", Format::Zstd),
];

fn suffix_of(name: &str) -> Option<(&'static str, Format)> {
    let bytes = name.as_bytes();
    SUFFIXES.iter().copied().find(|(suffix, _)| {
        bytes.len() >= suffix.len()
            && bytes[bytes.len() - suffix.len()..].eq_ignore_ascii_case(suffix.as_bytes())
    })
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Format::Gzip => "gzip",
            Format::Bzip2 => "bzip2",
            Format::Zstd => "zstd",
            other => other.extension(),
        })
    }
}
