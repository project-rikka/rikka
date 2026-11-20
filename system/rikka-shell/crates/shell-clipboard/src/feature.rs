use rikka_seam::Request;
use rikka_session::shortcuts::Shortcut;
use rikka_settings::session::clipboard::{
    self, HISTORIES, HISTORY, HISTORY_DEFAULT, PASTE, PASTE_DEFAULT,
};
use shell_kit::{EXIT_DURATION, Feature, Shell, SurfaceDesc};

use crate::history::History;
use crate::picker::Picker;
use crate::service::{CAPABILITY, ClipboardService, Pasting, TOGGLE};
use crate::view;

#[derive(Debug, Default)]
pub struct ClipboardManager;

impl ClipboardManager {
    #[must_use]
    pub fn new() -> ClipboardManager {
        ClipboardManager
    }
}

impl Feature for ClipboardManager {
    fn install(self, shell: Shell) -> Shell {
        let shell = shell.settings(clipboard::schema());
        let history = History::new(capacity(HISTORY_DEFAULT));
        let (shell, pasting) = shell.hold(Pasting {
            automatic: PASTE_DEFAULT,
        });
        let service = ClipboardService::new(shell.clipboard(), history.clone(), pasting);
        let presence = service.presence();
        let shell = shell.service(service);
        let seam = shell.seam();
        let aimed = shell.aimed();

        shell
            .on_shortcut(Shortcut::ClipboardHistory, Request::new(CAPABILITY, TOGGLE))
            .follow(clipboard::DOMAIN, HISTORY, {
                let history = history.clone();
                move |_, capacity: i64| history.set_capacity(self::capacity(capacity))
            })
            .follow(clipboard::DOMAIN, PASTE, move |host, on: bool| {
                host.get_mut(pasting).automatic = on;
            })
            .surface(
                SurfaceDesc::aimed(
                    aimed,
                    move |_, aim| Picker::new(seam.clone(), history.clone(), aim),
                    view::view,
                )
                .on_start(Picker::reread)
                .modal(presence, EXIT_DURATION),
            )
    }
}

fn capacity(value: i64) -> usize {
    value.clamp(*HISTORIES.start(), *HISTORIES.end()) as usize
}
