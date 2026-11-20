use rikka_emoji::Tone;
use rikka_seam::Request;
use rikka_session::shortcuts::Shortcut;
use rikka_settings::session::emoji::{self, SKIN_TONE, TYPE, TYPE_DEFAULT};
use rikka_types::Choice;
use shell_kit::{EXIT_DURATION, Feature, Recents, Shell, SurfaceDesc, TOGGLE};

use crate::picker::Picker;
use crate::service::{EmojiService, Inserting};
use crate::skin_tone::SkinTone;
use crate::{CAPABILITY, RECENTS, view};

#[derive(Debug, Default)]
pub struct Emoji;

impl Emoji {
    #[must_use]
    pub fn new() -> Emoji {
        Emoji
    }
}

impl Feature for Emoji {
    fn install(self, shell: Shell) -> Shell {
        let shell = shell.settings(emoji::schema::<Tone>());
        let (shell, inserting) = shell.hold(Inserting {
            typed: TYPE_DEFAULT,
        });
        let service = EmojiService::new(inserting);
        let presence = service.presence();
        let shell = shell.service(service);
        let seam = shell.seam();
        let aimed = shell.aimed();

        let skin_tone = SkinTone::default();
        let recents = Recents::read(RECENTS);

        shell
            .on_shortcut(Shortcut::EmojiPicker, Request::new(CAPABILITY, TOGGLE))
            .follow(emoji::DOMAIN, TYPE, move |host, on: bool| {
                host.get_mut(inserting).typed = on;
            })
            .follow(emoji::DOMAIN, SKIN_TONE, {
                let skin_tone = skin_tone.clone();
                move |_, name: String| {
                    if let Some(tone) = Tone::from_slug(&name) {
                        skin_tone.set(tone);
                    }
                }
            })
            .surface(
                SurfaceDesc::aimed(
                    aimed,
                    move |_, aim| {
                        Picker::new(seam.clone(), recents.clone(), skin_tone.clone(), aim)
                    },
                    view::view,
                )
                .on_start(Picker::start)
                .modal(presence, EXIT_DURATION),
            )
    }
}
