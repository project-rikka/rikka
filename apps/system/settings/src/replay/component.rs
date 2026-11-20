use rikka_app::prelude::*;
use rikka_audio::CaptureKind;
use rikka_settings::session::replay::{
    APPS, CODEC, CURSOR, CURSOR_DEFAULT, DOMAIN, ENABLED, ENABLED_DEFAULT, FOLDER, FPS,
    FPS_DEFAULT, FPSES, LENGTH, LENGTH_DEFAULT, LENGTH_STEP, LENGTHS, MEMORIES, MEMORY,
    MEMORY_DEFAULT, MEMORY_STEP, MICROPHONE, MICROPHONE_DEFAULT, MICROPHONE_DEVICE, QUALITIES,
    QUALITY, QUALITY_DEFAULT, SOUND, SOUND_DEFAULT, SUBJECT, apps,
};
use rikka_widgets::column;

use crate::capture::Devices;
use crate::controls::{Count, choice, flag_or, span};
use crate::domain::{Domain, domain_page};
use crate::notice::domain_notice;
use crate::page::Page;
use crate::pending::Pending;
use crate::picker::default_picker;
use crate::session::Session;

const SUBJECTS: [(&str, &str); 3] = [
    ("screen", "settings.replay.subjects.screen"),
    ("apps", "settings.replay.subjects.apps"),
    ("windows", "settings.replay.subjects.windows"),
];
const CODECS: [(&str, &str); 3] = [
    ("auto", "settings.replay.codecs.auto"),
    ("hevc", "settings.capture.codecs.hevc"),
    ("h264", "settings.capture.codecs.h264"),
];

const TYPED: f32 = 280.0;

pub(crate) struct Replay {
    domain: Domain,
    devices: Devices,
    apps: TextDocument,
    folder: TextDocument,
}

impl Page for Replay {
    fn opened(&mut self, _: &Pending, session: &Session) {
        self.domain.open::<Self>(session);
        self.devices.probe();
        let apps = self
            .domain
            .value(APPS)
            .map(|value| apps(&value))
            .unwrap_or_default();
        self.apps.set_text(apps.join(", "));
        self.folder
            .set_text(self.domain.get::<String>(FOLDER).unwrap_or_default());
    }
}

domain_page!(Replay);

impl Replay {
    pub(crate) fn new() -> Self {
        Self {
            domain: Domain::new(DOMAIN),
            devices: Devices::default(),
            apps: TextDocument::new(""),
            folder: TextDocument::new(""),
        }
    }

    fn is_on(&self) -> bool {
        self.domain.get::<bool>(ENABLED).unwrap_or(ENABLED_DEFAULT)
    }
}

pub(crate) fn view(replay: &Replay) -> impl View {
    let on = replay.is_on();
    column! {
        domain_notice(
            &replay.domain,
            "settings.replay.unavailable",
            "settings.replay.refused",
        );
        section(tr!("settings.replay.keeping"), keeping(replay));
        if on {
            section(tr!("settings.replay.picture"), picture(replay));
        }
        if on {
            section(tr!("settings.replay.sound"), sound(replay));
        }
    }
    .gap(Space::ROOMY)
}

fn keeping(replay: &Replay) -> impl View {
    let on = replay.is_on();
    let subject = replay.domain.one_of(SUBJECT, &SUBJECTS);
    column! {
        flag_or(replay, tr!("settings.replay.enabled"), ENABLED, ENABLED_DEFAULT);
        if on {
            choice(replay, tr!("settings.replay.subject"), SUBJECT, &SUBJECTS);
        }
        if on && subject == "apps" {
            field(tr!("settings.replay.apps"), chosen_apps(&replay.apps));
        }
        if on {
            Count {
                key: LENGTH,
                absent: LENGTH_DEFAULT,
                range: span(&LENGTHS),
            }
            .field_in_steps(replay, tr!("settings.replay.length"), LENGTH_STEP, |seconds| {
                rikka_format::format_duration(std::time::Duration::from_secs(seconds.unsigned_abs()))
            });
        }
        if on {
            Count {
                key: MEMORY,
                absent: MEMORY_DEFAULT,
                range: span(&MEMORIES),
            }
            .field_in_steps(replay, tr!("settings.replay.memory"), MEMORY_STEP, |mebibytes| {
                tr!("settings.replay.memory-reading", count = mebibytes).to_string()
            });
        }
        if on {
            field(tr!("settings.replay.folder"), folder(&replay.folder));
        }
    }
    .gap(Space::SNUG)
}

fn picture(replay: &Replay) -> impl View {
    column! {
        choice(replay, tr!("settings.replay.codec"), CODEC, &CODECS);
        Count {
            key: QUALITY,
            absent: QUALITY_DEFAULT,
            range: span(&QUALITIES),
        }
        .field(replay, tr!("settings.replay.quality"), |quality| quality.to_string());
        Count {
            key: FPS,
            absent: FPS_DEFAULT,
            range: span(&FPSES),
        }
        .field(replay, tr!("settings.replay.fps"), |fps| {
            tr!("settings.capture.fps-reading", count = fps).to_string()
        });
        flag_or(replay, tr!("settings.replay.cursor"), CURSOR, CURSOR_DEFAULT);
    }
    .gap(Space::SNUG)
}

fn sound(replay: &Replay) -> impl View {
    let microphone = replay
        .domain
        .get::<bool>(MICROPHONE)
        .unwrap_or(MICROPHONE_DEFAULT);
    let device = replay
        .domain
        .get::<String>(MICROPHONE_DEVICE)
        .unwrap_or_default();
    let kind = Some(CaptureKind::Microphone);
    column! {
        flag_or(replay, tr!("settings.replay.desktop-sound"), SOUND, SOUND_DEFAULT);
        flag_or(replay, tr!("settings.replay.microphone"), MICROPHONE, MICROPHONE_DEFAULT);
        if microphone {
            field(
                tr!("settings.replay.microphone-device"),
                default_picker(
                    tr!("settings.capture.audio-device-default"),
                    replay.devices.labels(kind),
                    replay.devices.at(kind, &device),
                    move |replay: &mut Replay, at: Option<usize>| {
                        let named = replay.devices.named(kind, at);
                        replay.domain.set(MICROPHONE_DEVICE, named);
                    },
                ),
            );
        }
    }
    .gap(Space::SNUG)
}

fn chosen_apps(document: &TextDocument) -> impl View {
    let commit = Handler::new(|replay: &mut Replay| {
        let value: Vec<String> = replay
            .apps
            .text()
            .split([',', ' ', '\n'])
            .map(str::trim)
            .filter(|app| !app.is_empty())
            .map(str::to_owned)
            .collect();
        let stored = replay
            .domain
            .value(APPS)
            .map(|value| apps(&value))
            .unwrap_or_default();
        if stored == value {
            skip_view();
            return;
        }
        replay.domain.set(APPS, value);
    });
    text_field(document)
        .placeholder(tr!("settings.replay.apps-hint"))
        .on_change(|| {})
        .on_blur(commit.clone())
        .on_submit(commit)
        .width(TYPED)
}

fn folder(document: &TextDocument) -> impl View {
    let commit = Handler::new(|replay: &mut Replay| {
        let value = replay.folder.text();
        if replay.domain.get::<String>(FOLDER).as_deref() == Some(value.as_str()) {
            skip_view();
            return;
        }
        replay.domain.set(FOLDER, value);
    });
    text_field(document)
        .placeholder(tr!("settings.replay.folder-default"))
        .on_change(|| {})
        .on_blur(commit.clone())
        .on_submit(commit)
        .width(TYPED)
}

impl std::fmt::Debug for Replay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Replay")
            .field("subject", &self.domain.one_of(SUBJECT, &SUBJECTS))
            .finish_non_exhaustive()
    }
}
