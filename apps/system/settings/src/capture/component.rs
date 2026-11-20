use rikka_app::prelude::*;
use rikka_audio::CaptureKind;
use rikka_settings::session::capture::{
    self as offered, AV1, DOMAIN, PICTURE_COPY, PICTURE_CURSOR, PICTURE_FOLDER, PICTURE_FORMAT,
    PICTURE_QUALITY, PICTURE_QUALITY_DEFAULT, PICTURE_SAVE, VIDEO_AUDIO, VIDEO_AUDIO_DEVICE,
    VIDEO_CODEC, VIDEO_CONTAINER, VIDEO_CURSOR, VIDEO_EFFORT, VIDEO_ENCODER, VIDEO_FOLDER,
    VIDEO_FPS, VIDEO_FPS_DEFAULT, VIDEO_QUALITY, VIDEO_QUALITY_DEFAULT, WEBM,
};
use rikka_widgets::column;

use crate::capture::devices::Devices;
use crate::controls::{Count, choice, flag, flag_or};
use crate::domain::{Domain, domain_page};
use crate::notice::domain_notice;
use crate::page::Page;
use crate::pending::Pending;
use crate::picker::{choose, default_picker};
use crate::session::Session;

const PICTURE_FORMATS: [(&str, &str); 4] = [
    ("png", "settings.capture.formats.png"),
    ("jpeg", "settings.capture.formats.jpeg"),
    ("webp", "settings.capture.formats.webp"),
    ("avif", "settings.capture.formats.avif"),
];
const CONTAINERS: [(&str, &str); 3] = [
    ("mp4", "settings.capture.containers.mp4"),
    ("mkv", "settings.capture.containers.mkv"),
    ("webm", "settings.capture.containers.webm"),
];
const CODECS: [(&str, &str); 3] = [
    ("h264", "settings.capture.codecs.h264"),
    ("hevc", "settings.capture.codecs.hevc"),
    ("av1", "settings.capture.codecs.av1"),
];
const ENCODERS: [(&str, &str); 7] = [
    ("auto", "settings.capture.encoders.auto"),
    ("nvenc", "settings.capture.encoders.nvenc"),
    ("amf", "settings.capture.encoders.amf"),
    ("qsv", "settings.capture.encoders.qsv"),
    ("vaapi", "settings.capture.encoders.vaapi"),
    ("vulkan", "settings.capture.encoders.vulkan"),
    ("software", "settings.capture.encoders.software"),
];
const EFFORTS: [(&str, &str); 4] = [
    ("fastest", "settings.capture.efforts.fastest"),
    ("fast", "settings.capture.efforts.fast"),
    ("balanced", "settings.capture.efforts.balanced"),
    ("slow", "settings.capture.efforts.slow"),
];
const AUDIO: [(&str, &str); 3] = [
    ("none", "settings.capture.sounds.none"),
    ("microphone", "settings.capture.sounds.microphone"),
    ("system", "settings.capture.sounds.system"),
];

const QUALITY: std::ops::RangeInclusive<f32> =
    *offered::QUALITY.start() as f32..=*offered::QUALITY.end() as f32;
const FPS: std::ops::RangeInclusive<f32> =
    *offered::FPS.start() as f32..=*offered::FPS.end() as f32;

const FOLDER: f32 = 220.0;

pub(crate) struct Capture {
    domain: Domain,
    devices: Devices,
    pictures: TextDocument,
    videos: TextDocument,
}

impl Page for Capture {
    fn opened(&mut self, _: &Pending, session: &Session) {
        self.domain.open::<Self>(session);
        self.devices.probe();
        self.pictures.set_text(
            self.domain
                .get::<String>(PICTURE_FOLDER)
                .unwrap_or_default(),
        );
        self.videos
            .set_text(self.domain.get::<String>(VIDEO_FOLDER).unwrap_or_default());
    }
}

domain_page!(Capture);

impl Capture {
    pub(crate) fn new() -> Self {
        Self {
            domain: Domain::new(DOMAIN),
            devices: Devices::default(),
            pictures: TextDocument::new(""),
            videos: TextDocument::new(""),
        }
    }

    fn audio_kind(&self) -> Option<CaptureKind> {
        match self.domain.one_of(VIDEO_AUDIO, &AUDIO) {
            "microphone" => Some(CaptureKind::Microphone),
            "system" => Some(CaptureKind::SystemAudio),
            _ => None,
        }
    }
}

pub(crate) fn view(capture: &Capture) -> impl View {
    column! {
        domain_notice(
            &capture.domain,
            "settings.capture.unavailable",
            "settings.capture.refused",
        );
        section(tr!("settings.capture.pictures"), pictures(capture));
        section(tr!("settings.capture.videos"), videos(capture));
    }
    .gap(Space::ROOMY)
}

fn pictures(capture: &Capture) -> impl View {
    let format = capture.domain.one_of(PICTURE_FORMAT, &PICTURE_FORMATS);
    column! {
        choice(
            capture,
            tr!("settings.capture.picture-format"),
            PICTURE_FORMAT,
            &PICTURE_FORMATS,
        );
        if format != "png" {
            quality(
                capture,
                tr!("settings.capture.picture-quality"),
                PICTURE_QUALITY,
                PICTURE_QUALITY_DEFAULT,
            );
        }
        flag_or(capture, tr!("settings.capture.picture-save"), PICTURE_SAVE, true);
        flag_or(capture, tr!("settings.capture.picture-copy"), PICTURE_COPY, true);
        flag(capture, tr!("settings.capture.picture-cursor"), PICTURE_CURSOR);
        field(
            tr!("settings.capture.picture-folder"),
            folder(&capture.pictures, PICTURE_FOLDER, |capture| &capture.pictures),
        );
    }
    .gap(Space::SNUG)
}

fn videos(capture: &Capture) -> impl View {
    let container = capture.domain.one_of(VIDEO_CONTAINER, &CONTAINERS);
    let codecs: &'static [(&'static str, &'static str)] = match container == WEBM {
        true => &CODECS[2..],
        false => &CODECS,
    };
    let codec = match container == WEBM {
        true => AV1,
        false => capture.domain.one_of(VIDEO_CODEC, &CODECS),
    };
    let kind = capture.audio_kind();
    let device = capture
        .domain
        .get::<String>(VIDEO_AUDIO_DEVICE)
        .unwrap_or_default();

    column! {
        choice(
            capture,
            tr!("settings.capture.container"),
            VIDEO_CONTAINER,
            &CONTAINERS,
        );
        field(
            tr!("settings.capture.codec"),
            choose(codecs, codec, |capture: &mut Capture, named: &'static str| {
                capture.domain.set(VIDEO_CODEC, named);
            }),
        );
        choice(capture, tr!("settings.capture.encoder"), VIDEO_ENCODER, &ENCODERS);
        quality(
            capture,
            tr!("settings.capture.video-quality"),
            VIDEO_QUALITY,
            VIDEO_QUALITY_DEFAULT,
        );
        choice(capture, tr!("settings.capture.effort"), VIDEO_EFFORT, &EFFORTS);
        Count {
            key: VIDEO_FPS,
            absent: VIDEO_FPS_DEFAULT,
            range: FPS,
        }
        .field(capture, tr!("settings.capture.fps"), |fps| {
            tr!("settings.capture.fps-reading", count = fps).to_string()
        });
        field(
            tr!("settings.capture.audio"),
            choose(
                &AUDIO,
                capture.domain.one_of(VIDEO_AUDIO, &AUDIO),
                |capture: &mut Capture, named: &'static str| {
                    capture.domain.set(VIDEO_AUDIO, named);
                    capture.domain.set(VIDEO_AUDIO_DEVICE, "");
                },
            ),
        );
        if kind.is_some() {
            field(
                tr!("settings.capture.audio-device"),
                default_picker(
                    tr!("settings.capture.audio-device-default"),
                    capture.devices.labels(kind),
                    capture.devices.at(kind, &device),
                    |capture: &mut Capture, at: Option<usize>| {
                        let named = capture.devices.named(capture.audio_kind(), at);
                        capture.domain.set(VIDEO_AUDIO_DEVICE, named);
                    },
                ),
            );
        }
        flag_or(capture, tr!("settings.capture.video-cursor"), VIDEO_CURSOR, true);
        field(
            tr!("settings.capture.video-folder"),
            folder(&capture.videos, VIDEO_FOLDER, |capture| &capture.videos),
        );
    }
    .gap(Space::SNUG)
}

fn quality(capture: &Capture, label: Label, key: &'static str, absent: i64) -> Field {
    Count {
        key,
        absent,
        range: QUALITY,
    }
    .field(capture, label, |quality| quality.to_string())
}

fn folder(
    document: &TextDocument,
    key: &'static str,
    typed: fn(&Capture) -> &TextDocument,
) -> impl View {
    let commit = Handler::new(move |capture: &mut Capture| {
        let value = typed(capture).text();
        if capture.domain.get::<String>(key).as_deref() == Some(value.as_str()) {
            skip_view();
            return;
        }
        capture.domain.set(key, value);
    });
    text_field(document)
        .placeholder(tr!("settings.capture.folder-default"))
        .on_change(|| {})
        .on_blur(commit.clone())
        .on_submit(commit)
        .width(FOLDER)
}

impl std::fmt::Debug for Capture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Capture")
            .field(
                "format",
                &self.domain.one_of(PICTURE_FORMAT, &PICTURE_FORMATS),
            )
            .finish_non_exhaustive()
    }
}
