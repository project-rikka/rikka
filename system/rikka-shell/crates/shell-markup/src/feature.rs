use shell_kit::{EXIT_DURATION, Feature, Shell, SurfaceDesc, Waiting, WaitingService};

use crate::editing::Editing;
use crate::editor::Editor;
use crate::service::AWAITED;
use crate::shared::Shared;
use crate::view;

#[derive(Debug)]
pub struct Markup {
    editing: Editing,
}

impl Markup {
    #[must_use]
    pub fn new(editing: Editing) -> Markup {
        Markup { editing }
    }
}

impl Feature for Markup {
    fn install(self, shell: Shell) -> Shell {
        let waiting = Waiting::default();
        let service = WaitingService::new(AWAITED, waiting.clone());
        let presence = service.presence();
        let shell = shell.service(service);

        let shared = Shared::new(shell.seam(), self.editing, waiting);
        shell.surface(
            SurfaceDesc::new(move || Editor::new(shared.clone()), view::view)
                .on_start(Editor::start)
                .name("markup")
                .modal(presence, EXIT_DURATION),
        )
    }
}
