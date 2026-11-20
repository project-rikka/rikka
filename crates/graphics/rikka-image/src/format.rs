use std::fmt;
use std::path::Path;

use crate::color_type::ColorType;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Format {
    Png,
    Jpeg,
    WebP,
    Gif,
    Avif,
    Heif,
    Bmp,
    Ico,
    Tiff,
    Jpeg2000,
    JpegXl,
    Tga,
    Qoi,
    Dds,
    Hdr,
    OpenExr,
    Farbfeld,
    Pnm,
}

const FILE_TYPE: &[u8; 4] = b"ftyp";
const AVIF_BRANDS: [&[u8; 4]; 2] = [b"avif", b"avis"];
const HEIF_BRANDS: [&[u8; 4]; 9] = [
    b"heic", b"heix", b"heim", b"heis", b"hevc", b"hevx", b"mif1", b"mif2", b"msf1",
];

impl Format {
    pub const ALL: [Format; 18] = [
        Format::Png,
        Format::Jpeg,
        Format::WebP,
        Format::Avif,
        Format::Heif,
        Format::JpegXl,
        Format::Gif,
        Format::Tiff,
        Format::Jpeg2000,
        Format::Bmp,
        Format::Ico,
        Format::Qoi,
        Format::Tga,
        Format::OpenExr,
        Format::Hdr,
        Format::Pnm,
        Format::Farbfeld,
        Format::Dds,
    ];

    #[must_use]
    pub fn detect(bytes: &[u8]) -> Option<Format> {
        if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
            return Some(Format::WebP);
        }
        if bytes.get(4..8) == Some(FILE_TYPE) {
            return brand_family(bytes);
        }
        SIGNATURES
            .iter()
            .find(|signature| signature.matches(bytes))
            .map(|signature| signature.format)
    }

    #[must_use]
    pub fn from_path(path: impl AsRef<Path>) -> Option<Format> {
        path.as_ref()
            .extension()
            .and_then(|extension| extension.to_str())
            .and_then(Format::from_extension)
    }

    #[must_use]
    pub fn from_extension(extension: &str) -> Option<Format> {
        let extension = extension.strip_prefix('.').unwrap_or(extension);
        Format::ALL.into_iter().find(|format| {
            format
                .accepted_extensions()
                .iter()
                .any(|known| known.eq_ignore_ascii_case(extension))
        })
    }

    #[must_use]
    pub fn from_mime_type(mime_type: &str) -> Option<Format> {
        Format::ALL.into_iter().find(|format| {
            format
                .mime_types()
                .iter()
                .any(|known| known.eq_ignore_ascii_case(mime_type))
        })
    }

    #[must_use]
    pub fn extension(self) -> &'static str {
        self.extensions()[0]
    }

    #[must_use]
    pub const fn extensions(self) -> &'static [&'static str] {
        match self {
            Format::Png => &["png"],
            Format::Jpeg => &["jpg", "jpeg"],
            Format::WebP => &["webp"],
            Format::Gif => &["gif"],
            Format::Avif => &["avif"],
            Format::Heif => &["heic", "heif", "hif"],
            Format::Bmp => &["bmp"],
            Format::Ico => &["ico"],
            Format::Tiff => &["tiff", "tif"],
            Format::Jpeg2000 => &["jp2", "j2k", "jpg2", "j2c", "jpc"],
            Format::JpegXl => &["jxl"],
            Format::Tga => &["tga"],
            Format::Qoi => &["qoi"],
            Format::Dds => &["dds"],
            Format::Hdr => &["hdr"],
            Format::OpenExr => &["exr"],
            Format::Farbfeld => &["ff"],
            Format::Pnm => &["pnm", "ppm", "pgm", "pbm", "pam"],
        }
    }

    const fn accepted_extensions(self) -> &'static [&'static str] {
        match self {
            Format::Png => &["png", "apng"],
            Format::Jpeg => &["jpg", "jpeg", "jfif"],
            Format::Avif => &["avif", "avifs"],
            other => other.extensions(),
        }
    }

    #[must_use]
    pub fn mime_type(self) -> &'static str {
        self.mime_types()[0]
    }

    #[must_use]
    pub const fn mime_types(self) -> &'static [&'static str] {
        match self {
            Format::Png => &["image/png"],
            Format::Jpeg => &["image/jpeg"],
            Format::WebP => &["image/webp"],
            Format::Gif => &["image/gif"],
            Format::Avif => &["image/avif", "image/avif-sequence"],
            Format::Heif => &[
                "image/heif",
                "image/heic",
                "image/heif-sequence",
                "image/heic-sequence",
            ],
            Format::Bmp => &["image/bmp"],
            Format::Ico => &["image/x-icon", "image/vnd.microsoft.icon"],
            Format::Tiff => &["image/tiff"],
            Format::Jpeg2000 => &["image/jp2", "image/x-jp2-codestream"],
            Format::JpegXl => &["image/jxl"],
            Format::Tga => &["image/x-targa", "image/x-tga"],
            Format::Qoi => &["image/x-qoi", "image/qoi"],
            Format::Dds => &["image/vnd-ms.dds"],
            Format::Hdr => &["image/vnd.radiance"],
            Format::OpenExr => &["image/x-exr"],
            Format::Farbfeld => &["application/octet-stream"],
            Format::Pnm => &[
                "image/x-portable-anymap",
                "image/x-portable-bitmap",
                "image/x-portable-graymap",
                "image/x-portable-pixmap",
            ],
        }
    }

    #[must_use]
    pub const fn can_encode(self) -> bool {
        !matches!(self, Format::Dds | Format::Heif | Format::Jpeg2000)
    }

    #[must_use]
    pub const fn supports_alpha(self) -> bool {
        !matches!(self, Format::Jpeg | Format::Hdr | Format::Pnm)
    }

    pub(crate) const fn needs_ffmpeg(self) -> bool {
        matches!(
            self,
            Format::Avif
                | Format::Heif
                | Format::JpegXl
                | Format::Tiff
                | Format::Jpeg2000
                | Format::OpenExr
        )
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Format::Png => "PNG",
            Format::Jpeg => "JPEG",
            Format::WebP => "WebP",
            Format::Gif => "GIF",
            Format::Avif => "AVIF",
            Format::Heif => "HEIF",
            Format::Bmp => "BMP",
            Format::Ico => "ICO",
            Format::Tiff => "TIFF",
            Format::Jpeg2000 => "JPEG 2000",
            Format::JpegXl => "JPEG XL",
            Format::Tga => "TGA",
            Format::Qoi => "QOI",
            Format::Dds => "DDS",
            Format::Hdr => "Radiance HDR",
            Format::OpenExr => "OpenEXR",
            Format::Farbfeld => "farbfeld",
            Format::Pnm => "PNM",
        }
    }

    #[must_use]
    pub(crate) fn encode_color(self, color: ColorType) -> ColorType {
        let gray = !color.has_color();
        let alpha = color.has_alpha();
        let deep = color.bits_per_channel() > 8;

        match self {
            Format::Png
            | Format::Pnm
            | Format::Tiff
            | Format::Avif
            | Format::Heif
            | Format::Jpeg2000 => ColorType::integer(gray, alpha, deep),
            Format::Bmp | Format::Tga => ColorType::integer(gray, alpha, false),
            Format::WebP | Format::Ico | Format::Qoi | Format::Dds => {
                ColorType::integer(false, alpha, false)
            }
            Format::Gif => ColorType::Rgba8,
            Format::Jpeg => ColorType::integer(gray, false, false),
            Format::OpenExr if alpha => ColorType::Rgba32F,
            Format::OpenExr | Format::Hdr => ColorType::Rgb32F,
            Format::Farbfeld => ColorType::Rgba16,
            Format::JpegXl => color,
        }
    }
}

fn brand_family(bytes: &[u8]) -> Option<Format> {
    let declared = bytes
        .first_chunk::<4>()
        .map_or(0, |size| u32::from_be_bytes(*size) as usize);
    let end = declared.min(bytes.len());
    let major = bytes.get(8..12).into_iter();
    let (compatible, _) = bytes.get(16..end).unwrap_or_default().as_chunks::<4>();
    let brands: Vec<&[u8]> = major
        .chain(compatible.iter().map(|brand| brand.as_slice()))
        .collect();
    let carries = |family: &[&[u8; 4]]| {
        brands
            .iter()
            .any(|brand| family.iter().any(|known| known.as_slice() == *brand))
    };
    if carries(&AVIF_BRANDS) {
        return Some(Format::Avif);
    }
    carries(&HEIF_BRANDS).then_some(Format::Heif)
}

struct Signature {
    format: Format,
    magic: &'static [u8],
}

impl Signature {
    const fn at_start(format: Format, magic: &'static [u8]) -> Signature {
        Signature { format, magic }
    }

    fn matches(&self, bytes: &[u8]) -> bool {
        bytes.starts_with(self.magic)
    }
}

const SIGNATURES: [Signature; 25] = [
    Signature::at_start(Format::Png, b"\x89PNG\r\n\x1a\n"),
    Signature::at_start(Format::Jpeg, &[0xFF, 0xD8, 0xFF]),
    Signature::at_start(Format::Gif, b"GIF89a"),
    Signature::at_start(Format::Gif, b"GIF87a"),
    Signature::at_start(Format::Tiff, b"MM\0*"),
    Signature::at_start(Format::Tiff, b"II*\0"),
    Signature::at_start(Format::Jpeg2000, b"\0\0\0\x0CjP  \r\n\x87\n"),
    Signature::at_start(Format::Jpeg2000, &[0xFF, 0x4F, 0xFF, 0x51]),
    Signature::at_start(Format::JpegXl, b"\0\0\0\x0CJXL \r\n\x87\n"),
    Signature::at_start(Format::JpegXl, &[0xFF, 0x0A]),
    Signature::at_start(Format::Dds, b"DDS "),
    Signature::at_start(Format::Bmp, b"BM"),
    Signature::at_start(Format::Ico, &[0, 0, 1, 0]),
    Signature::at_start(Format::Hdr, b"#?RADIANCE"),
    Signature::at_start(Format::Hdr, b"#?RGBE"),
    Signature::at_start(Format::Farbfeld, b"farbfeld"),
    Signature::at_start(Format::OpenExr, &[0x76, 0x2F, 0x31, 0x01]),
    Signature::at_start(Format::Qoi, b"qoif"),
    Signature::at_start(Format::Pnm, b"P1"),
    Signature::at_start(Format::Pnm, b"P2"),
    Signature::at_start(Format::Pnm, b"P3"),
    Signature::at_start(Format::Pnm, b"P4"),
    Signature::at_start(Format::Pnm, b"P5"),
    Signature::at_start(Format::Pnm, b"P6"),
    Signature::at_start(Format::Pnm, b"P7"),
];

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}
