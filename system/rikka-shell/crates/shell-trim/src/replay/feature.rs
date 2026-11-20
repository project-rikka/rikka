use rikka_seam::Request;
use rikka_session::shortcuts::{self, Shortcut};
use shell_kit::{EXIT_DURATION, Feature, OPEN, PresenceService, Shell, SurfaceDesc};

use crate::replay::chooser::{CAPABILITY, Chooser, SUBJECT, SUMMARY, Shared};
use crate::replay::{saving, view};

#[derive(Debug, Default)]
pub struct InstantReplay;

impl InstantReplay {
    #[must_use]
    pub fn new() -> InstantReplay {
        InstantReplay
    }
}

impl Feature for InstantReplay {
    fn install(self, shell: Shell) -> Shell {
        let service = PresenceService::new(CAPABILITY, SUMMARY, SUBJECT);
        let presence = service.presence();
        let shell = shell.service(service);
        let shared = Shared::new(shell.seam());
        shell
            .on_shortcuts(|shortcut| {
                (shortcut == Shortcut::ChooseReplay).then(|| Request::new(CAPABILITY, OPEN))
            })
            .watch(shortcuts::CAPABILITY, |host, incoming| {
                let saving = incoming
                    .event()
                    .and_then(shortcuts::activated)
                    .is_some_and(|shortcut| shortcut == Shortcut::SaveReplay);
                if saving {
                    saving::save_now(host.seam().clone());
                }
            })
            .surface(
                SurfaceDesc::new(move || Chooser::new(shared.clone()), view::view)
                    .on_start(Chooser::start)
                    .name("replay")
                    .modal(presence, EXIT_DURATION),
            )
    }
}
