use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use rikka_audio::CaptureSource;
use rikka_image::Format;
use rikka_settings::seam::Settings;
use rikka_video::{EncodeSpeed, EncoderChoice, VideoCodec};
use shell_kit::Shell;

use crate::settings::{self, Container};

#[derive(Clone, Debug)]
pub(crate) struct Picture {
    pub(crate) format: Format,
    pub(crate) quality: u8,
    pub(crate) save: bool,
    pub(crate) copy: bool,
    pub(crate) folder: Option<PathBuf>,
    pub(crate) cursor: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct Video {
    pub(crate) container: Container,
    pub(super) codec: VideoCodec,
    pub(crate) backend: EncoderChoice,
    pub(crate) quality: u8,
    pub(crate) speed: EncodeSpeed,
    pub(super) fps: u32,
    audio_kind: String,
    audio_device: String,
    pub(crate) cursor: bool,
    pub(crate) folder: Option<PathBuf>,
}

impl Video {
    #[must_use]
    pub(super) fn codec(&self) -> VideoCodec {
        self.container.holding(self.codec)
    }

    #[must_use]
    pub(crate) fn audio(&self) -> Option<CaptureSource> {
        settings::audio(&self.audio_kind, &self.audio_device)
    }
}

#[derive(Clone, Debug)]
struct Values {
    picture: Picture,
    video: Video,
}

#[derive(Clone, Debug)]
pub(crate) struct Preferences(Rc<RefCell<Values>>);

impl Preferences {
    #[must_use]
    pub(crate) fn picture(&self) -> Picture {
        self.0.borrow().picture.clone()
    }

    #[must_use]
    pub(crate) fn video(&self) -> Video {
        self.0.borrow().video.clone()
    }
}

impl Values {
    fn read(store: Option<&Settings>) -> Values {
        use settings as key;

        let text = |name: &str, absent: &str| {
            store
                .and_then(|store| store.read::<String>(name))
                .unwrap_or_else(|| absent.to_owned())
        };
        let count = |name: &str, absent: i64| {
            store
                .and_then(|store| store.read::<i64>(name))
                .unwrap_or(absent)
        };
        let flag = |name: &str, absent: bool| {
            store
                .and_then(|store| store.read::<bool>(name))
                .unwrap_or(absent)
        };

        let audio_kind = text(key::VIDEO_AUDIO, key::VIDEO_AUDIO_DEFAULT);
        let audio_device = text(key::VIDEO_AUDIO_DEVICE, "");
        Values {
            picture: Picture {
                format: key::picture_format(&text(
                    key::PICTURE_FORMAT,
                    key::PICTURE_FORMAT_DEFAULT,
                )),
                quality: key::quality(count(key::PICTURE_QUALITY, key::PICTURE_QUALITY_DEFAULT)),
                save: flag(key::PICTURE_SAVE, key::PICTURE_SAVE_DEFAULT),
                copy: flag(key::PICTURE_COPY, key::PICTURE_COPY_DEFAULT),
                folder: key::folder(&text(key::PICTURE_FOLDER, "")),
                cursor: flag(key::PICTURE_CURSOR, key::PICTURE_CURSOR_DEFAULT),
            },
            video: Video {
                container: key::container(&text(
                    key::VIDEO_CONTAINER,
                    key::VIDEO_CONTAINER_DEFAULT,
                )),
                codec: key::codec(&text(key::VIDEO_CODEC, key::VIDEO_CODEC_DEFAULT)),
                backend: key::encoder(&text(key::VIDEO_ENCODER, key::VIDEO_ENCODER_DEFAULT)),
                quality: key::quality(count(key::VIDEO_QUALITY, key::VIDEO_QUALITY_DEFAULT)),
                speed: key::effort(&text(key::VIDEO_EFFORT, key::VIDEO_EFFORT_DEFAULT)),
                fps: key::fps(count(key::VIDEO_FPS, key::VIDEO_FPS_DEFAULT)),
                audio_kind,
                audio_device,
                cursor: flag(key::VIDEO_CURSOR, key::VIDEO_CURSOR_DEFAULT),
                folder: key::folder(&text(key::VIDEO_FOLDER, "")),
            },
        }
    }
}

pub(crate) fn follow(shell: Shell) -> (Shell, Preferences) {
    let current = shell
        .reader(settings::DOMAIN)
        .map(|reader| Values::read(Some(&reader.borrow())))
        .unwrap_or_else(|| Values::read(None));
    let preferences = Preferences(Rc::new(RefCell::new(current)));
    let shell = shell.on_domain(settings::DOMAIN, {
        let preferences = preferences.clone();
        move |_, store| *preferences.0.borrow_mut() = Values::read(Some(store))
    });
    (shell, preferences)
}
