use rikka_polkit::Agent;
use rikka_seam::Request;
use shell_kit::{Feature, KeyboardMode, Shell, SurfaceDesc};

use crate::dialog::Dialog;
use crate::service::{CAPABILITY, PolkitService, REPORT};
use crate::view;

#[derive(Debug, Default)]
pub struct PolkitAgent;

impl PolkitAgent {
    #[must_use]
    pub fn new() -> PolkitAgent {
        PolkitAgent
    }
}

impl Feature for PolkitAgent {
    fn install(self, shell: Shell) -> Shell {
        let seam = shell.seam();
        let reporting = seam.clone();
        let agent = match Agent::start(move |asking| {
            reporting.notify(Request::new(CAPABILITY, REPORT).encode(&asking));
        }) {
            Ok(agent) => agent,
            Err(error) => {
                tracing::warn!("polkit agent unavailable: {error}");
                return shell;
            }
        };

        let service = PolkitService::new(agent);
        let presence = service.presence();
        let agent = service.agent();
        shell.service(service).surface(
            SurfaceDesc::new(move || Dialog::new(seam.clone(), agent.clone()), view::view)
                .on_start(Dialog::start)
                .overlay()
                .scrim()
                .keyboard(KeyboardMode::Exclusive)
                .animated(presence, view::EXIT_DURATION),
        )
    }
}
