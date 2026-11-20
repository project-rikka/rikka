mod describe;
mod service;
mod watch;

use service::SystemUpdateService;
use shell_kit::{Feature, Shell};
use watch::Inbox;

#[derive(Debug, Default)]
pub struct SystemUpdates;

impl Feature for SystemUpdates {
    fn install(self, shell: Shell) -> Shell {
        let inbox = Inbox::default();
        let seam = shell.seam();
        watch::watch(inbox.clone(), seam);
        shell.service(SystemUpdateService::new(inbox))
    }
}
