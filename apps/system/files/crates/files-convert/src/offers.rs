use rikka_mime::MimeType;

use crate::kind::Kind;
use crate::picture::is_drawing;
use crate::strip::can_strip;
use crate::target::{ModelFormat, MovieFormat, PictureFormat, SoundFormat, Target, extension_of};
use crate::tuning::Scale;

const SHRINKS: [Scale; 4] = [
    Scale::Half,
    Scale::LongestSide(1920),
    Scale::LongestSide(1280),
    Scale::LongestSide(640),
];

#[must_use]
pub fn offers<'a>(sources: impl IntoIterator<Item = (&'a str, &'a MimeType)>) -> Vec<Target> {
    let mut kind = None;
    let mut names = Vec::new();
    for (name, mime) in sources {
        let Some(this) = Kind::detect(name, mime) else {
            return Vec::new();
        };
        if kind.is_some_and(|seen| seen != this) {
            return Vec::new();
        }
        kind = Some(this);
        names.push(name);
    }
    let Some(kind) = kind else {
        return Vec::new();
    };
    candidates(kind, &names)
        .into_iter()
        .filter(|target| !names.iter().all(|name| target.matches_format(name)))
        .collect()
}

fn candidates(kind: Kind, names: &[&str]) -> Vec<Target> {
    let all = |test: fn(&str) -> bool| names.iter().all(|name| test(name));
    match kind {
        Kind::Picture => {
            let mut targets: Vec<Target> = PictureFormat::ALL.map(Target::Picture).into();
            if all(shrinkable) {
                targets.extend(SHRINKS.map(Target::Shrink));
            }
            if all(can_strip) {
                targets.push(Target::StripMetadata);
            }
            targets
        }
        Kind::Sound => SoundFormat::ALL.map(Target::Sound).into(),
        Kind::Movie => MovieFormat::ALL
            .map(Target::Movie)
            .into_iter()
            .chain(SoundFormat::AUDIO_EXTRACTION.map(Target::Extract))
            .collect(),
        Kind::Model => ModelFormat::ALL
            .into_iter()
            .filter(|format| *format != ModelFormat::Vrm || all(is_avatar))
            .map(Target::Model)
            .collect(),
    }
}

fn shrinkable(name: &str) -> bool {
    !is_drawing(std::path::Path::new(name))
        && rikka_image::Format::from_extension(extension_of(name))
            .is_some_and(rikka_image::Format::can_encode)
}

fn is_avatar(name: &str) -> bool {
    extension_of(name).eq_ignore_ascii_case("vrm")
}
