use std::path::PathBuf;

use rikka_capture::{Region, Source};
use rikka_dirs::UserDir;
use rikka_record::{AudioTrack, RecordConfig, RecordSummary, Recording};
use rikka_seam::{Request, SeamClient};
use shell_kit::detached;
use shell_notifications::Announcement;

use crate::error::Result;
use crate::preferences::Video;
use crate::service::{CAPABILITY, SETTLE};
use crate::store;

const FOLDER: &str = "Screencasts";

const STEM: &str = "Screencast";

#[derive(Clone, Debug)]
pub(crate) struct Strings {
    pub(crate) app_name: String,
    pub(crate) started: String,
    pub(crate) saved: String,
    pub(crate) failed: String,
}

#[derive(Clone, Debug)]
pub(crate) enum Subject {
    Output { name: String, part: Option<Region> },
    Window(String),
}

impl Subject {
    fn source(&self) -> Source {
        match self {
            Subject::Output { name, .. } if name.is_empty() => Source::FirstOutput,
            Subject::Output { name, .. } => Source::Output(name.clone()),
            Subject::Window(id) => Source::Window(id.clone()),
        }
    }

    fn part(&self) -> Option<Region> {
        match self {
            Subject::Output { part, .. } => *part,
            Subject::Window(_) => None,
        }
    }
}

pub(crate) fn start(
    subject: &Subject,
    video: &Video,
    seam: SeamClient,
) -> Result<(Recording, PathBuf)> {
    let folder = store::folder(video.folder.as_deref(), UserDir::Videos, FOLDER)?;
    store::ensure(&folder)?;
    let file = store::file(&folder, STEM, video.container.extension());

    let mut config = RecordConfig::default()
        .fps(video.fps)
        .codec(video.codec())
        .backend(video.backend)
        .quality(video.quality)
        .speed(video.speed)
        .cursor(video.cursor);
    if let Some(part) = subject.part() {
        config = config.region(part);
    }
    if let Some(source) = video.audio() {
        config = config.audio(AudioTrack::new(source));
    }

    let recording = Recording::start(subject.source(), &file, config, move || {
        seam.notify(Request::new(CAPABILITY, SETTLE));
    })?;
    tracing::info!("recording {subject:?} to {}", file.display());
    Ok((recording, file))
}

pub(crate) fn finish(recording: Recording, words: Strings, seam: SeamClient) {
    detached("rikka-capture-record", move || match recording.stop() {
        Ok(summary) => saved(&words, &summary).send(&seam),
        Err(error) => {
            tracing::warn!("recording did not finish cleanly: {error}");
            Announcement::new(&words.app_name, &words.failed)
                .body(error.to_string())
                .urgent()
                .send(&seam);
        }
    });
}

pub(crate) fn began(words: &Strings, file: &std::path::Path) -> Announcement {
    Announcement::new(&words.app_name, &words.started).file(file)
}

fn saved(words: &Strings, summary: &RecordSummary) -> Announcement {
    Announcement::new(&words.app_name, &words.saved)
        .file(&summary.path)
        .action_request(shell_trim::action_request(&summary.path))
}
