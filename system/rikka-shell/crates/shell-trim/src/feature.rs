use shell_kit::{EXIT_DURATION, Feature, Shell, SurfaceDesc, Waiting, WaitingService};

use crate::editor::Editor;
use crate::service::AWAITED;
use crate::shared::Shared;
use crate::view;

#[derive(Debug, Default)]
pub struct Trim;

impl Trim {
    #[must_use]
    pub fn new() -> Trim {
        Trim
    }
}

impl Feature for Trim {
    fn install(self, shell: Shell) -> Shell {
        let waiting = Waiting::default();
        let service = WaitingService::new(AWAITED, waiting.clone());
        let presence = service.presence();
        let shell = shell.service(service);

        let shared = Shared::new(shell.seam(), waiting);
        shell.surface(
            SurfaceDesc::new(move || Editor::new(shared.clone()), view::view)
                .on_start(Editor::start)
                .name("trim")
                .modal(presence, EXIT_DURATION),
        )
    }
}
