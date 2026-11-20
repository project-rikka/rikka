use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::channel;
use std::time::Duration;

use cut_export::{
    Audio, Job, Order, Phase, Range, Resolution, SoundCodec, Span, Still, StillFormat, Summary,
    Video,
};
use cut_project::Project;
use rikka_audio_decode::WavDepth;
use rikka_video::EncoderChoice;

use crate::support::{Engine, SAMPLE_RATE, click, engine, film, placed, sound};

const TIMEOUT: Duration = Duration::from_secs(50);

fn finished(engine: &Engine, order: Order) -> Phase {
    let (sender, heard) = channel();
    let mut job = Job::start(order, engine.imaging.clone(), move |report| {
        let _ = sender.send(report);
    })
    .expect("a started export");
    while !job.is_over() {
        job.hear(
            heard
                .recv_timeout(TIMEOUT)
                .expect("the export reports in time"),
        );
    }
    job.phase().clone()
}

fn done(phase: Phase) -> Summary {
    match phase {
        Phase::Done(summary) => summary,
        other => panic!("the export ended as {other:?}"),
    }
}

fn names(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(folder)
        .expect("a readable folder")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

fn filmed(folder: &Path, frames: u32) -> Option<Project> {
    let media = film(&folder.join("footage.mp4"), (64, 36), 30, frames)?;
    Some(placed(&Project::new(), media, 0, 0).0)
}

fn software() -> Video {
    Video {
        encoder: EncoderChoice::Only(rikka_video::EncoderBackend::Software),
        ..Video::default()
    }
}

#[test]
fn video_export_writes_all_frames_and_sound() {
    let engine = engine();
    let folder = tempfile::tempdir().expect("a folder");
    let Some(project) = filmed(folder.path(), 15) else {
        return;
    };
    let span = Span::resolve(&project, Range::Whole).expect("something to export");
    let path = folder.path().join("export.mp4");
    let summary = done(finished(
        &engine,
        Order::video(Arc::new(project), span, software(), path.clone()),
    ));

    assert_eq!(summary.frames, 15);
    assert!(summary.sound);
    assert_eq!((summary.undecodable, summary.offline), (0, 0));
    let video = rikka_video::probe(&path).expect("a readable video");
    assert_eq!((video.width, video.height), (64, 36));
    assert!((video.frame_rate - 30.0).abs() < 1e-6);
    let decoded = rikka_audio_decode::read(path.as_path())
        .probe()
        .expect("a sound track");
    assert_eq!((decoded.sample_rate, decoded.channels), (SAMPLE_RATE, 2));
    let length = decoded.duration.expect("a sound length").as_secs_f64();
    assert!((length - 0.5).abs() < 0.03, "{length}");
    assert_eq!(names(folder.path()), ["export.mp4", "footage.mp4"]);
}

#[test]
fn audio_export_writes_marked_range() {
    let engine = engine();
    let folder = tempfile::tempdir().expect("a folder");
    let media = sound(&folder.path().join("click.wav"), &click(48_000, 24_000), 1);
    let (project, _) = placed(&Project::new(), media, 0, 0);
    let project = project
        .apply(&cut_project::Edit::Mark {
            point: cut_project::Point::In,
            at: 10,
        })
        .expect("marked")
        .0;
    let span = Span::resolve(&project, Range::suggested(&project)).expect("something to export");
    assert_eq!((span.first(), span.end()), (10, 30));
    let audio = Audio {
        codec: SoundCodec::Wav,
        depth: WavDepth::Pcm24,
        ..Audio::default()
    };
    let path = folder.path().join("mix.wav");
    done(finished(
        &engine,
        Order::audio(Arc::new(project), span, audio, path.clone()),
    ));

    let decoded = rikka_audio_decode::read(path.as_path())
        .decode()
        .expect("a WAV file");
    assert_eq!(decoded.frames(), 32_000);
    let loudest = decoded
        .samples()
        .as_chunks::<2>()
        .0
        .iter()
        .enumerate()
        .max_by(|a, b| a.1[0].abs().total_cmp(&b.1[0].abs()))
        .map(|(at, _)| at);
    assert_eq!(loudest, Some(24_000 - 16_000));
}
#[test]
fn export_overwrites_only_when_requested() {
    let engine = engine();
    let folder = tempfile::tempdir().expect("a folder");
    let media = sound(&folder.path().join("tone.wav"), &click(4800, 0), 1);
    let project = Arc::new(placed(&Project::new(), media, 0, 0).0);
    let span = Span::resolve(&project, Range::Whole).expect("something to export");
    let path = folder.path().join("mix.wav");
    for (overwrite, expected_name, earlier) in
        [(false, "mix (2).wav", true), (true, "mix.wav", false)]
    {
        std::fs::write(&path, b"earlier").expect("an earlier file");
        let order = Order::audio(Arc::clone(&project), span, Audio::default(), path.clone());
        let order = match overwrite {
            true => order.overwriting(),
            false => order,
        };
        let summary = done(finished(&engine, order));

        assert_eq!(summary.path, folder.path().join(expected_name));
        let preserved = std::fs::read(&path).expect("the target") == b"earlier";
        assert_eq!(preserved, earlier, "overwrite {overwrite}");
    }
}

#[test]
fn still_export_renders_frame_at_size() {
    let engine = engine();
    let folder = tempfile::tempdir().expect("a folder");
    let Some(project) = filmed(folder.path(), 6) else {
        return;
    };
    let still = Still {
        format: StillFormat::Png,
        resolution: Resolution::Short(18),
    };
    let path = folder.path().join("still.png");
    done(finished(
        &engine,
        Order::still(Arc::new(project), 3, still, path.clone()),
    ));

    let image = rikka_image::read(path.as_path())
        .decode()
        .expect("a picture");
    assert_eq!((image.width(), image.height()), (32, 18));
    let pixels = image.rgba8();
    let centre = ((9 * 32 + 16) * 4) as usize;
    let [red, green, blue] = [0, 1, 2].map(|channel| pixels[centre + channel]);
    assert!(red > 170 && green < 80 && blue < 80, "{red} {green} {blue}");
}

#[test]
fn cancelled_export_keeps_existing_file() {
    let engine = engine();
    let folder = tempfile::tempdir().expect("a folder");
    let Some(project) = filmed(folder.path(), 90) else {
        return;
    };
    let path: PathBuf = folder.path().join("kept.mp4");
    std::fs::write(&path, b"earlier").expect("an earlier file");
    let span = Span::resolve(&project, Range::Whole).expect("something to export");
    let (sender, heard) = channel();
    let mut job = Job::start(
        Order::video(Arc::new(project), span, software(), path.clone()),
        engine.imaging.clone(),
        move |report| {
            let _ = sender.send(report);
        },
    )
    .expect("a started export");
    job.cancel();
    while !job.is_over() {
        job.hear(
            heard
                .recv_timeout(TIMEOUT)
                .expect("the export reports in time"),
        );
    }

    assert!(
        matches!(job.phase(), Phase::Cancelled),
        "the export ended as {:?}",
        job.phase()
    );
    assert_eq!(std::fs::read(&path).expect("the earlier file"), b"earlier");
    assert_eq!(names(folder.path()), ["footage.mp4", "kept.mp4"]);
}
