use rikka_seam::SeamClient;
use rikka_session::toplevels::ToplevelsClient;
use rikka_ui::State;

use crate::tasks::menu::opened::{MenuRequest, Opened};
use crate::tasks::state::ask;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MenuAction {
    Focus(String),
    Start,
    Maximize(String),
    Unmaximize(String),
    Minimize(String),
    Move { workspace: usize },
    Close,
}

#[derive(Debug, State)]
pub(crate) struct Flyout {
    seam: SeamClient,
    menu: Opened,
    request: Option<MenuRequest>,
}

impl Flyout {
    #[must_use]
    pub(crate) fn new(seam: SeamClient, menu: &Opened) -> Flyout {
        Flyout {
            seam,
            menu: menu.clone(),
            request: menu.request(),
        }
    }

    #[must_use]
    pub(crate) fn request(&self) -> Option<&MenuRequest> {
        self.request.as_ref()
    }

    pub(crate) fn dismiss(&mut self) {
        self.menu.close();
    }

    pub(crate) fn act(&mut self, action: MenuAction) {
        self.menu.close();
        let Some(request) = &self.request else {
            return;
        };
        let toplevels = ToplevelsClient::new(self.seam.clone());
        match action {
            MenuAction::Focus(id) => ask(toplevels.focus(id)),
            MenuAction::Start => {
                if let Some(spawn) = &request.starts {
                    ask(self.seam.call(spawn.request()));
                }
            }
            MenuAction::Maximize(id) => ask(toplevels.maximize(id)),
            MenuAction::Unmaximize(id) => ask(toplevels.unmaximize(id)),
            MenuAction::Minimize(id) => ask(toplevels.minimize(id)),
            MenuAction::Move { workspace } => {
                for window in &request.windows {
                    ask(toplevels.move_to_workspace(window.id.clone(), workspace));
                }
            }
            MenuAction::Close => {
                for window in &request.windows {
                    toplevels.notify_close(window.id.clone());
                }
            }
        }
    }
}
