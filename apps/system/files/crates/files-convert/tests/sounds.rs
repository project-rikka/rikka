use std::path::Path;
use std::time::Duration;

use files_convert::{Request, SoundFormat, Target, Tuning, convert};
use lofty::config::WriteOptions;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::{Accessor, Tag, TagExt, TagType};
use rikka_audio_decode::{WavDepth, WavWriter};

use crate::support::{names, succeeded};

const RATE: u32 = 44_100;

fn take(path: &Path, seconds: u32) {
    let mut writer = WavWriter::create(path, RATE, 2, WavDepth::Pcm24).expect("create wav");
    let samples: Vec<f32> = (0..RATE * seconds)
        .flat_map(|frame| {
            let at = frame as f32 / RATE as f32;
            let sample = (at * 330.0 * std::f32::consts::TAU).sin() * 0.4;
            [sample, sample]
        })
        .collect();
    writer.write(&samples).expect("write samples");
    writer.finish().expect("finish wav");
}

fn titled(path: &Path, title: &str) {
    let mut tag = Tag::new(TagType::Id3v2);
    tag.set_title(title.to_owned());
    tag.save_to_path(path, WriteOptions::default())
        .expect("write tags");
}

fn title(path: &Path) -> Option<String> {
    let read = lofty::read_from_path(path).expect("read tagged file");
    let tag = read.primary_tag().or_else(|| read.first_tag())?;
    tag.title().map(|title| title.into_owned())
}

#[test]
fn sound_formats_keep_duration_and_tags() {
    let folder = tempfile::tempdir().unwrap();
    let source = folder.path().join("take.wav");
    take(&source, 1);
    titled(&source, "Night Walk");

    for format in SoundFormat::ALL {
        if format == SoundFormat::Wav {
            continue;
        }
        succeeded(&[&source], Target::Sound(format), "");
        let written = folder.path().join(format!("take.{}", format.extension()));
        let info = rikka_audio_decode::read(written.as_path())
            .probe()
            .unwrap_or_else(|error| panic!("{format:?}: {error}"));
        let length = info.duration.expect("known duration");
        assert!(
            length.abs_diff(Duration::from_secs(1)) < Duration::from_millis(100),
            "{format:?} lasts {length:?}"
        );
        assert_eq!(title(&written).as_deref(), Some("Night Walk"), "{format:?}");
        if matches!(format, SoundFormat::Flac | SoundFormat::Alac) {
            let depth = lofty::read_from_path(&written)
                .expect("read lossless file")
                .properties()
                .bit_depth();
            assert_eq!(depth, Some(24), "{format:?}");
        }
        std::fs::remove_file(&written).expect("remove output");
    }
}

#[test]
fn cancelled_conversion_leaves_no_files() {
    let folder = tempfile::tempdir().unwrap();
    let source = folder.path().join("long.wav");
    take(&source, 20);

    let job = convert(Request {
        sources: vec![source.clone()],
        target: Target::Sound(SoundFormat::Flac),
        tuning: Tuning::default(),
        suffix: String::new(),
    });
    job.cancel();
    let outcome = job.wait();

    assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    let left = names(folder.path());
    if outcome.cancelled {
        assert_eq!(left, ["long.wav"]);
    } else {
        assert_eq!(left, ["long.flac", "long.wav"]);
    }
}
