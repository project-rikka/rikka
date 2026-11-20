use std::path::Path;

use rikka_archive::Format;

pub const OFFERS: [Format; 7] = [
    Format::Zip,
    Format::SevenZ,
    Format::TarGzip,
    Format::TarXz,
    Format::TarZstd,
    Format::TarBzip2,
    Format::Tar,
];

pub(crate) fn file_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}
