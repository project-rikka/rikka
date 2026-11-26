use rikka_llm::Effort;
use rikka_seam::Request;
use rikka_session::shortcuts::Shortcut;
use rikka_settings::Schema;
use rikka_settings::session::translate;
use shell_capture::{CAPABILITY as CAPTURE, Mode, PICK, Snip};
use shell_kit::{EXIT_DURATION, Feature, Shell, SurfaceDesc, TOGGLE};

use crate::profile::Profile;
use crate::prompts::Prompts;
use crate::service::{CAPABILITY, TranslateService};
use crate::shared::Shared;
use crate::window::Translate;
use crate::{choices, view};

#[derive(Debug)]
pub struct Translator {
    snip: Snip,
}

impl Translator {
    #[must_use]
    pub fn new(snip: Snip) -> Translator {
        Translator { snip }
    }

    #[must_use]
    pub fn schema() -> Schema {
        translate::schema::<Profile>(Effort::ALL.iter().map(|effort| effort.id().to_owned()))
    }
}

impl Feature for Translator {
    fn install(self, shell: Shell) -> Shell {
        let prompts = match Prompts::load() {
            Ok(prompts) => prompts,
            Err(error) => {
                tracing::error!("translation unavailable: {error}");
                return shell;
            }
        };

        let shell = shell.settings(Self::schema());
        let (shell, choices) = choices::follow(shell);

        let service = TranslateService::new();
        let presence = service.presence();
        let shell = shell.service(service);

        let shared = Shared::new(shell.seam(), prompts, choices, self.snip);
        shell
            .on_shortcuts(|shortcut| match shortcut {
                Shortcut::ToggleTranslate => Some(Request::new(CAPABILITY, TOGGLE)),
                Shortcut::TranslateRegion => {
                    Some(Request::new(CAPTURE, PICK).encode(&Mode::Translate))
                }
                _ => None,
            })
            .surface(
                SurfaceDesc::new(move || Translate::new(shared.clone()), view::view)
                    .on_start(Translate::start)
                    .name("translate")
                    .modal(presence, EXIT_DURATION)
                    .on_leave(Translate::leave),
            )
    }
}
