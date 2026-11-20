use std::path::Path;

use rikka_audio_decode::AudioFormat;
use rikka_image::Format as Picture;

use crate::format::Format;
use crate::header;

const PNG: Format = Format::of_picture(Picture::Png).measured(header::png);
const JPEG: Format = Format::of_picture(Picture::Jpeg).measured(header::jpeg);
const GIF: Format = Format::of_picture(Picture::Gif).measured(header::gif);
const WEBP: Format = Format::of_picture(Picture::WebP).measured(header::webp);
const BMP: Format = Format::of_picture(Picture::Bmp).measured(header::bmp);
const ICO: Format = Format::of_picture(Picture::Ico).measured(header::ico);
const TIFF: Format = Format::of_picture(Picture::Tiff).measured(header::tiff);
const TGA: Format = Format::of_picture(Picture::Tga).measured(header::tga);
const QOI: Format = Format::of_picture(Picture::Qoi).measured(header::qoi);
const DDS: Format = Format::of_picture(Picture::Dds).measured(header::dds);
const RADIANCE: Format = Format::of_picture(Picture::Hdr).measured(header::radiance);
const PNM: Format = Format::of_picture(Picture::Pnm).measured(header::pnm);
const FARBFELD: Format = Format::of_picture(Picture::Farbfeld).measured(header::farbfeld);
const EXR: Format = Format::of_picture(Picture::OpenExr);
const AVIF: Format = Format::of_picture(Picture::Avif);
const HEIF: Format = Format::of_picture(Picture::Heif);
const JPEG2000: Format = Format::of_picture(Picture::Jpeg2000);
const JPEG_XL: Format = Format::of_picture(Picture::JpegXl);

const MP3: Format = Format::of_sound("MP3", Some(AudioFormat::Mp3));
const FLAC: Format = Format::of_sound("FLAC", Some(AudioFormat::Flac));
const WAV: Format = Format::of_sound("WAV", Some(AudioFormat::Wav));
const OGG: Format = Format::of_sound("Ogg", Some(AudioFormat::Ogg));
const VORBIS: Format = Format::of_sound("Ogg Vorbis", Some(AudioFormat::Ogg));
const OPUS: Format = Format::of_sound("Opus", Some(AudioFormat::Opus));
const AAC: Format = Format::of_sound("AAC", Some(AudioFormat::Aac));
const M4A: Format = Format::of_sound("M4A", Some(AudioFormat::Mp4));
const AIFF: Format = Format::of_sound("AIFF", Some(AudioFormat::Aiff));
const CAF: Format = Format::of_sound("CAF", Some(AudioFormat::Caf));
const MKA: Format = Format::of_sound("Matroska Audio", Some(AudioFormat::Mkv));
const WMA: Format = Format::of_sound("WMA", Some(AudioFormat::Wma));
const WAVPACK: Format = Format::of_sound("WavPack", Some(AudioFormat::WavPack));
const APE: Format = Format::of_sound("Monkey's Audio", Some(AudioFormat::Ape));
const DSF: Format = Format::of_sound("DSF", Some(AudioFormat::Dsf));
const DFF: Format = Format::of_sound("DSDIFF", Some(AudioFormat::Dff));
const TAK: Format = Format::of_sound("TAK", Some(AudioFormat::Tak));
const MUSEPACK: Format = Format::of_sound("Musepack", Some(AudioFormat::Musepack));
const AC3: Format = Format::of_sound("AC-3", Some(AudioFormat::Ac3));
const EAC3: Format = Format::of_sound("E-AC-3", Some(AudioFormat::Eac3));
const TTA: Format = Format::of_sound("TTA", Some(AudioFormat::Tta));

const MP4: Format = Format::of_movie("MP4");
const MATROSKA: Format = Format::of_movie("Matroska");
const WEBM: Format = Format::of_movie("WebM");
const QUICKTIME: Format = Format::of_movie("QuickTime");
const AVI: Format = Format::of_movie("AVI");
const MPEG: Format = Format::of_movie("MPEG");
const MPEG_TS: Format = Format::of_movie("MPEG-TS");
const FLV: Format = Format::of_movie("FLV");
const THEORA: Format = Format::of_movie("Ogg Theora");
const ASF: Format = Format::of_movie("ASF");
const THREE_GPP: Format = Format::of_movie("3GPP");

struct Pattern {
    at: usize,
    bytes: &'static [u8],
}

const fn at(at: usize, bytes: &'static [u8]) -> Pattern {
    Pattern { at, bytes }
}

const SIGNATURES: &[(&[Pattern], Format)] = &[
    (&[at(0, b"\x89PNG\r\n\x1a\n")], PNG),
    (&[at(0, b"\xFF\xD8\xFF")], JPEG),
    (&[at(0, b"GIF8")], GIF),
    (&[at(0, b"RIFF"), at(8, b"WEBP")], WEBP),
    (&[at(0, b"RIFF"), at(8, b"WAVE")], WAV),
    (&[at(0, b"RIFF"), at(8, b"AVI ")], AVI),
    (&[at(0, b"\x00\x00\x01\x00")], ICO),
    (&[at(0, b"II*\x00")], TIFF),
    (&[at(0, b"MM\x00*")], TIFF),
    (&[at(0, b"\x00\x00\x00\x0CjP  \r\n\x87\n")], JPEG2000),
    (&[at(0, b"\xFF\x4F\xFF\x51")], JPEG2000),
    (&[at(0, b"\x00\x00\x00\x0CJXL \r\n\x87\n")], JPEG_XL),
    (&[at(0, b"\xFF\x0A")], JPEG_XL),
    (&[at(0, b"qoif")], QOI),
    (&[at(0, b"DDS ")], DDS),
    (&[at(0, b"#?RADIANCE")], RADIANCE),
    (&[at(0, b"#?RGBE")], RADIANCE),
    (&[at(0, b"\x76\x2f\x31\x01")], EXR),
    (&[at(0, b"farbfeld")], FARBFELD),
    (&[at(0, b"BM")], BMP),
    (&[at(4, b"ftyp"), at(8, b"M4A ")], M4A),
    (&[at(4, b"ftyp"), at(8, b"M4B ")], M4A),
    (&[at(4, b"ftyp"), at(8, b"qt  ")], QUICKTIME),
    (&[at(4, b"ftyp"), at(8, b"3g")], THREE_GPP),
    (&[at(4, b"ftyp")], MP4),
    (&[at(0, b"OggS"), at(28, b"\x80theora")], THEORA),
    (&[at(0, b"OggS"), at(28, b"OpusHead")], OPUS),
    (&[at(0, b"OggS"), at(28, b"\x01vorbis")], VORBIS),
    (&[at(0, b"OggS")], OGG),
    (&[at(0, b"fLaC")], FLAC),
    (&[at(0, b"FORM"), at(8, b"AIF")], AIFF),
    (&[at(0, b"caff")], CAF),
    (&[at(0, b"wvpk")], WAVPACK),
    (&[at(0, b"MAC ")], APE),
    (&[at(0, b"DSD "), at(28, b"fmt ")], DSF),
    (&[at(0, b"FRM8"), at(12, b"DSD ")], DFF),
    (&[at(0, b"tBaK")], TAK),
    (&[at(0, b"MPCK")], MUSEPACK),
    (&[at(0, b"MP+")], MUSEPACK),
    (&[at(0, b"TTA1")], TTA),
    (&[at(0, b"ID3")], MP3),
    (&[at(0, b"\xFF\xFB")], MP3),
    (&[at(0, b"\xFF\xFA")], MP3),
    (&[at(0, b"\xFF\xF3")], MP3),
    (&[at(0, b"\xFF\xF2")], MP3),
    (&[at(0, b"\xFF\xF1")], AAC),
    (&[at(0, b"\xFF\xF9")], AAC),
    (&[at(0, b"\x0B\x77")], AC3),
    (&[at(0, b"\x1A\x45\xDF\xA3")], MATROSKA),
    (&[at(0, b"\x30\x26\xB2\x75\x8E\x66\xCF\x11")], ASF),
    (&[at(0, b"FLV\x01")], FLV),
    (&[at(0, b"\x00\x00\x01\xBA")], MPEG),
    (&[at(0, b"\x00\x00\x01\xB3")], MPEG),
    (&[at(0, b"G"), at(188, b"G"), at(376, b"G")], MPEG_TS),
];

const EXTENSIONS: &[(&str, Format)] = &[
    ("png", PNG),
    ("apng", PNG),
    ("jpg", JPEG),
    ("jpeg", JPEG),
    ("jpe", JPEG),
    ("jfif", JPEG),
    ("gif", GIF),
    ("webp", WEBP),
    ("bmp", BMP),
    ("dib", BMP),
    ("ico", ICO),
    ("cur", ICO),
    ("tif", TIFF),
    ("tiff", TIFF),
    ("tga", TGA),
    ("targa", TGA),
    ("qoi", QOI),
    ("dds", DDS),
    ("hdr", RADIANCE),
    ("exr", EXR),
    ("avif", AVIF),
    ("avifs", AVIF),
    ("heic", HEIF),
    ("heif", HEIF),
    ("hif", HEIF),
    ("jp2", JPEG2000),
    ("jpg2", JPEG2000),
    ("j2k", JPEG2000),
    ("j2c", JPEG2000),
    ("jpc", JPEG2000),
    ("jxl", JPEG_XL),
    ("ff", FARBFELD),
    ("pbm", PNM),
    ("pgm", PNM),
    ("ppm", PNM),
    ("pnm", PNM),
    ("pam", PNM),
    ("mp3", MP3),
    ("mp2", MP3),
    ("mpga", MP3),
    ("flac", FLAC),
    ("wav", WAV),
    ("wave", WAV),
    ("ogg", VORBIS),
    ("oga", OGG),
    ("opus", OPUS),
    ("aac", AAC),
    ("m4a", M4A),
    ("m4b", M4A),
    ("aiff", AIFF),
    ("aif", AIFF),
    ("aifc", AIFF),
    ("caf", CAF),
    ("mka", MKA),
    ("wma", WMA),
    ("wv", WAVPACK),
    ("ape", APE),
    ("dsf", DSF),
    ("dff", DFF),
    ("tak", TAK),
    ("mpc", MUSEPACK),
    ("mp+", MUSEPACK),
    ("mpp", MUSEPACK),
    ("ac3", AC3),
    ("eac3", EAC3),
    ("ec3", EAC3),
    ("tta", TTA),
    ("mp4", MP4),
    ("m4v", MP4),
    ("mkv", MATROSKA),
    ("webm", WEBM),
    ("mov", QUICKTIME),
    ("qt", QUICKTIME),
    ("avi", AVI),
    ("mpg", MPEG),
    ("mpeg", MPEG),
    ("mpe", MPEG),
    ("m2v", MPEG),
    ("vob", MPEG),
    ("m2ts", MPEG_TS),
    ("mts", MPEG_TS),
    ("flv", FLV),
    ("ogv", THEORA),
    ("ogm", THEORA),
    ("wmv", ASF),
    ("asf", ASF),
    ("3gp", THREE_GPP),
    ("3g2", THREE_GPP),
];

pub(crate) fn identify(path: &Path, header: &[u8]) -> Option<Format> {
    match by_signature(header) {
        Some(format) => Some(disambiguate(format, path)),
        None => by_name(path),
    }
}

pub(crate) fn by_name(path: &Path) -> Option<Format> {
    let extension = path.extension()?.to_str()?;
    EXTENSIONS
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(extension))
        .map(|(_, format)| *format)
}

fn by_signature(header: &[u8]) -> Option<Format> {
    if let Some(picture) = picture_in_file_type(header) {
        return Some(picture);
    }
    SIGNATURES
        .iter()
        .find(|(patterns, _)| patterns.iter().all(|pattern| matches(header, pattern)))
        .map(|(_, format)| *format)
}

fn picture_in_file_type(header: &[u8]) -> Option<Format> {
    match Picture::detect(header)? {
        Picture::Avif => Some(AVIF),
        Picture::Heif => Some(HEIF),
        _ => None,
    }
}

fn disambiguate(format: Format, path: &Path) -> Format {
    let named = |name: &str| {
        path.extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case(name))
    };
    if format == MATROSKA {
        if named("mka") {
            return MKA;
        }
        if named("webm") {
            return WEBM;
        }
    }
    if format == ASF && named("wma") {
        return WMA;
    }
    if format == AC3 && (named("eac3") || named("ec3")) {
        return EAC3;
    }
    format
}

fn matches(header: &[u8], pattern: &Pattern) -> bool {
    header
        .get(pattern.at..pattern.at + pattern.bytes.len())
        .is_some_and(|found| found == pattern.bytes)
}
