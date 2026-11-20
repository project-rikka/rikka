use rikka_seam::Request;
use rikka_settings::session::lock::{AT_LOGIN, DOMAIN};
use shell_kit::{Feature, KeyboardMode, SessionLock, Shell, SurfaceDesc};

use crate::keyring::dialog::Dialog;
use crate::keyring::service::{CAPABILITY, KeyringService, REPORT, Showing};
use crate::keyring::{view, watch};

#[derive(Debug)]
pub struct KeyringPrompt {
    lock: SessionLock,
}

impl KeyringPrompt {
    #[must_use]
    pub fn new(lock: SessionLock) -> KeyringPrompt {
        KeyringPrompt { lock }
    }
}

impl Feature for KeyringPrompt {
    fn install(self, shell: Shell) -> Shell {
        let seam = shell.seam();
        let showing = Showing::new(self.lock.clone());
        self.lock.on_change({
            let showing = showing.clone();
            move |_| showing.settle()
        });
        let reporting = seam.clone();
        watch::watch(move |report| {
            reporting.notify(Request::new(CAPABILITY, REPORT).encode(&report));
        });
        let presence = showing.presence();
        shell
            .service(KeyringService::new(showing))
            .follow(DOMAIN, AT_LOGIN, |_, at_login: bool| {
                watch::set_auto_unlock(!at_login);
            })
            .surface(
                SurfaceDesc::new(move || Dialog::new(seam.clone()), view::view)
                    .on_start(Dialog::start)
                    .overlay()
                    .scrim()
                    .keyboard(KeyboardMode::Exclusive)
                    .animated(presence, view::EXIT_DURATION),
            )
    }
}
