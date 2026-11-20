use rikka_mime::MimeType;

use crate::target::{ModelFormat, extension_of};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Kind {
    Picture,
    Sound,
    Movie,
    Model,
}

impl Kind {
    pub(crate) fn detect(name: &str, mime: &MimeType) -> Option<Kind> {
        let extension = extension_of(name);
        if ModelFormat::ALL
            .iter()
            .any(|format| extension.eq_ignore_ascii_case(format.extension()))
        {
            return Some(Kind::Model);
        }
        match mime.kind() {
            "image" if is_picture(mime, extension) => Some(Kind::Picture),
            "audio" if rikka_audio_decode::AudioFormat::from_extension(extension).is_some() => {
                Some(Kind::Sound)
            }
            "video" => Some(Kind::Movie),
            _ => None,
        }
    }
}

fn is_picture(mime: &MimeType, extension: &str) -> bool {
    if mime.subtype() == "svg+xml" {
        return true;
    }
    rikka_image::Format::from_mime_type(mime.as_str())
        .or_else(|| rikka_image::Format::from_extension(extension))
        .is_some()
}
