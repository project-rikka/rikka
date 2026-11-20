use files_convert::{ModelFormat, MovieFormat, PictureFormat, Scale, SoundFormat, Target, offers};

use crate::support::mime;

type Selection<'a> = &'a [(&'a str, &'a str)];

fn offered(sources: Selection<'_>) -> Vec<Target> {
    let types: Vec<_> = sources.iter().map(|(_, essence)| mime(essence)).collect();
    offers(
        sources
            .iter()
            .zip(&types)
            .map(|((name, _), mime)| (*name, mime)),
    )
}

#[test]
fn offers_targets_except_source_format() {
    let cases: [(Selection<'_>, &[Target], &[Target]); 7] = [
        (
            &[("photo.png", "image/png")],
            &[
                Target::Picture(PictureFormat::Jpeg),
                Target::Picture(PictureFormat::WebP),
                Target::Shrink(Scale::LongestSide(1920)),
                Target::StripMetadata,
            ],
            &[Target::Picture(PictureFormat::Png)],
        ),
        (
            &[("logo.svg", "image/svg+xml")],
            &[Target::Picture(PictureFormat::Png)],
            &[Target::Shrink(Scale::Half), Target::StripMetadata],
        ),
        (
            &[("a.png", "image/png"), ("b.jpg", "image/jpeg")],
            &[
                Target::Picture(PictureFormat::Png),
                Target::Picture(PictureFormat::Jpeg),
            ],
            &[],
        ),
        (
            &[("take.wav", "audio/x-wav")],
            &[
                Target::Sound(SoundFormat::Mp3),
                Target::Sound(SoundFormat::Flac),
                Target::Sound(SoundFormat::Aac),
            ],
            &[Target::Sound(SoundFormat::Wav)],
        ),
        (
            &[("clip.mp4", "video/mp4")],
            &[
                Target::Movie(MovieFormat::WebM),
                Target::Movie(MovieFormat::Gif),
                Target::Extract(SoundFormat::Mp3),
            ],
            &[Target::Movie(MovieFormat::Mp4)],
        ),
        (
            &[("textured_model.glb", "model/gltf-binary")],
            &[
                Target::Model(ModelFormat::Gltf),
                Target::Model(ModelFormat::Pmx),
            ],
            &[
                Target::Model(ModelFormat::Glb),
                Target::Model(ModelFormat::Vrm),
            ],
        ),
        (
            &[("avatar.vrm", "application/octet-stream")],
            &[Target::Model(ModelFormat::Vrm)],
            &[],
        ),
    ];

    for (sources, present, absent) in cases {
        let targets = offered(sources);
        for target in present {
            assert!(targets.contains(target), "{sources:?} lacks {target:?}");
        }
        for target in absent {
            assert!(!targets.contains(target), "{sources:?} offers {target:?}");
        }
    }
}

#[test]
fn mixed_selection_offers_nothing() {
    for sources in [
        &[("photo.png", "image/png"), ("take.wav", "audio/x-wav")][..],
        &[("notes.txt", "text/plain")][..],
        &[][..],
    ] {
        assert!(offered(sources).is_empty(), "{sources:?}");
    }
}
