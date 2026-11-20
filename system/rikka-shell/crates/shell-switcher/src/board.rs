use std::sync::Arc;

use rikka_seam::SeamClient;
use rikka_session::switcher::{self, Rows, Switch, SwitcherClient};
use rikka_types::Size;
use rikka_ui::{IconSource, State, stream};
use shell_apps::{Apps, owner};
use shell_kit::{Placement, Presence};
use switcher_layout::{Arrangement, arrange};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Face {
    pub(crate) id: Arc<str>,
    pub(crate) title: Arc<str>,
    pub(crate) icon: IconSource,
}

#[derive(Debug)]
pub(crate) struct Shown {
    pub(crate) switch: Switch,
    pub(crate) laid: Arc<Arrangement>,
    pub(super) faces: Arc<[Face]>,
}

#[derive(State)]
pub(crate) struct Board {
    seam: SeamClient,
    apps: Apps,
    presence: Presence,
    place: Placement,
    shown: Option<Shown>,
}

impl Board {
    #[must_use]
    pub(crate) fn new(seam: SeamClient, apps: Apps, presence: Presence, place: Placement) -> Board {
        Board {
            seam,
            apps,
            presence,
            place,
            shown: None,
        }
    }

    pub(crate) fn start(&mut self) {
        stream(switcher::states(&self.seam), Board::event);
    }

    #[must_use]
    pub(crate) fn shown(&self) -> Option<&Shown> {
        self.shown.as_ref()
    }

    pub(crate) fn pointed(&mut self, index: usize) {
        let client = self.client();
        let Some(shown) = &mut self.shown else {
            return;
        };
        let Some(window) = shown.switch.windows.get(index) else {
            return;
        };
        if shown.switch.selected != index {
            client.select(shown.switch.serial, window.id.as_str());
            shown.switch.selected = index;
        }
    }

    pub(crate) fn chose(&mut self, index: usize) {
        if let Some(shown) = &self.shown
            && let Some(window) = shown.switch.windows.get(index)
        {
            self.client()
                .choose(shown.switch.serial, window.id.as_str());
        }
    }

    pub(crate) fn dismissed(&mut self) {
        if let Some(shown) = &self.shown {
            self.client().dismiss(shown.switch.serial);
        }
    }

    fn event(&mut self, switch: Option<Switch>) {
        let Some(switch) = switch else {
            self.presence.set(false);
            return;
        };
        if let Some(shown) = &mut self.shown
            && shown.switch.serial == switch.serial
            && shown.switch.windows == switch.windows
        {
            shown.switch.selected = switch.selected;
            return;
        }
        let sizes: Vec<Size> = switch
            .windows
            .iter()
            .map(|window| Size::new(window.width as f32, window.height as f32))
            .collect();
        let laid = Arc::new(arrange(self.place.size, &sizes));
        self.client().arrange(switch.serial, Rows(laid.centres()));
        let faces = self.faces(&switch);
        self.shown = Some(Shown {
            switch,
            laid,
            faces,
        });
    }

    fn faces(&self, switch: &Switch) -> Arc<[Face]> {
        let icons = self.apps.icons();
        self.apps.installed().with(|catalogue| {
            switch
                .windows
                .iter()
                .map(|window| {
                    let entry = owner(catalogue.entries(), &window.app_id);
                    let icon = icons.for_app(entry, &window.app_id);
                    let title = match (window.title.is_empty(), entry) {
                        (false, _) => window.title.as_str(),
                        (true, Some(entry)) => entry.name.as_str(),
                        (true, None) => window.app_id.as_str(),
                    };
                    Face {
                        id: Arc::from(window.id.as_str()),
                        title: Arc::from(title),
                        icon,
                    }
                })
                .collect()
        })
    }

    fn client(&self) -> SwitcherClient {
        SwitcherClient::new(self.seam.clone())
    }
}

impl std::fmt::Debug for Board {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Board")
            .field("output", &self.place.name)
            .field(
                "shown",
                &self.shown.as_ref().map(|shown| shown.switch.serial),
            )
            .finish_non_exhaustive()
    }
}
