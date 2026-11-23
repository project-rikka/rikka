use futures_util::StreamExt;
use rikka_bluetooth::DeviceId;
use rikka_network::ApId;
use rikka_power::Action;
use rikka_session::settings_app::{self, Link, Section};
use rikka_ui::{State, TextDocument, after, skip_view, stream};
use rikka_widgets::Dismissible;
use shell_feeds::Daemons;
use shell_kit::launch::Spawn;
use shell_notifications::{History, HistoryList};

use crate::centre::detail::Detail;
use crate::shared::Shared;
#[derive(Debug, State)]
pub(crate) struct Centre {
    shared: Shared,
    page: Option<Detail>,
    joining: Option<ApId>,
    passphrase: TextDocument,
    notifications: HistoryList,
}

impl Centre {
    #[must_use]
    pub(crate) fn new(shared: &Shared) -> Centre {
        Centre {
            shared: shared.clone(),
            page: None,
            joining: None,
            passphrase: TextDocument::default(),
            notifications: HistoryList::new(&shared.history),
        }
    }

    pub(crate) fn start(&mut self) {
        self.shared.history.mark_read();
        let daemons = self.daemons();
        stream(daemons.sound().changes().map(|_| ()), moved);
        stream(daemons.network().changes().map(|_| ()), moved);
        stream(daemons.bluetooth().changes().map(|_| ()), moved);
        stream(daemons.power().changes().map(|_| ()), moved);
        stream(daemons.battery().main_changes().map(|_| ()), moved);
        stream(
            self.shared.history.changes(),
            |centre: &mut Centre, (): ()| {
                if !centre.follow_history() {
                    skip_view();
                }
            },
        );
    }

    #[must_use]
    pub(crate) fn notifications(&self) -> &HistoryList {
        &self.notifications
    }

    pub(crate) fn forget(&mut self, id: u32) {
        self.shared.history.forget(id);
        self.follow_history();
    }

    pub(crate) fn clear_notifications(&mut self) {
        self.shared.history.clear();
        self.follow_history();
    }

    fn follow_history(&mut self) -> bool {
        let followed = self.notifications.follow(&self.shared.history);
        for key in followed.left {
            after(Dismissible::DEPARTURE, move |centre: &mut Centre| {
                centre.notifications.remove(key);
            });
        }
        followed.changed
    }

    #[must_use]
    pub(crate) fn daemons(&self) -> &Daemons {
        &self.shared.daemons
    }

    #[must_use]
    pub(crate) fn page(&self) -> Option<Detail> {
        self.page
    }

    #[must_use]
    pub(super) fn is_quiet(&self) -> bool {
        self.shared.quiet.get()
    }

    #[must_use]
    fn history(&self) -> &History {
        &self.shared.history
    }

    #[must_use]
    pub(crate) fn joining(&self) -> Option<&ApId> {
        self.joining.as_ref()
    }

    #[must_use]
    pub(crate) fn passphrase(&self) -> &TextDocument {
        &self.passphrase
    }

    #[must_use]
    pub(crate) fn below(&self) -> f32 {
        self.shared.taskbar_visibility.below()
    }

    #[must_use]
    pub(crate) fn border(&self) -> f32 {
        self.shared.taskbar_visibility.border()
    }

    pub(crate) fn dismiss(&mut self) {
        self.shared.centre.set(false);
    }

    pub(crate) fn open(&mut self, page: Option<Detail>) {
        self.page = page;
        self.joining = None;
        self.passphrase = TextDocument::default();
        if page == Some(Detail::Network) {
            self.daemons().network().look();
        }
    }

    pub(crate) fn join(&mut self, ap: &ApId) {
        let network = self.daemons().network().state();
        let Some(point) = network.networks().into_iter().find(|point| &point.id == ap) else {
            return;
        };
        if point.in_use {
            return;
        }
        if point.security.needs_account() {
            return self.open_settings(Section::Network.into());
        }
        if point.saved.is_some() || point.security.is_open() {
            self.daemons().network().join(point, String::new());
            self.joining = None;
            return;
        }
        self.joining = Some(point.id.clone());
        self.passphrase = TextDocument::default();
    }

    pub(super) fn confirm(&mut self) {
        let network = self.daemons().network().state();
        let Some(point) = self
            .joining
            .as_ref()
            .and_then(|ap| network.networks().into_iter().find(|point| &point.id == ap))
        else {
            return;
        };
        self.daemons().network().join(point, self.passphrase.text());
        self.joining = None;
        self.passphrase = TextDocument::default();
    }

    pub(crate) fn cancel(&mut self) {
        self.joining = None;
        self.passphrase = TextDocument::default();
    }

    pub(super) fn mute(&mut self, node: u32) {
        let sound = self.daemons().sound();
        let Some(muted) = sound.state().node(node).map(|node| node.volume.muted()) else {
            return;
        };
        sound.mute(node, !muted);
    }

    pub(crate) fn connect(&mut self, device: &DeviceId, connected: bool) {
        self.daemons().bluetooth().connect(device, connected);
    }

    pub(super) fn airplane(&mut self, on: bool) {
        let daemons = self.daemons();
        if daemons.network().state().radio().is_some() {
            daemons.network().use_wireless(!on);
        }
        if daemons.bluetooth().state().radio().is_some() {
            daemons.bluetooth().use_radio(!on);
        }
    }

    pub(crate) fn quiet(&mut self, quiet: bool) {
        self.shared.quiet.ask(&self.shared.seam, quiet);
    }

    pub(crate) fn ask(&mut self, action: Action) {
        self.daemons().power().ask(action);
        self.dismiss();
    }

    pub(crate) fn settings(&mut self) {
        self.spawn(Spawn::new([settings_app::PROGRAM]));
    }

    pub(crate) fn open_settings(&mut self, link: Link) {
        self.spawn(Spawn::settings(link));
    }

    pub(crate) fn activate(&mut self, id: u32) {
        let handed = self.history().activate(id);
        self.deliver(handed);
    }

    pub(crate) fn act(&mut self, id: u32, key: &str) {
        let handed = self.history().invoke(id, key);
        self.deliver(handed);
    }

    fn deliver(&mut self, handed: bool) {
        self.follow_history();
        if handed {
            self.dismiss();
        }
    }

    pub(crate) fn follow(&mut self, id: u32, at: usize) {
        let Some(href) = self.history().link(id, at) else {
            return;
        };
        self.spawn(Spawn::new(rikka_open::opener(&href)));
    }

    fn spawn(&mut self, spawn: Spawn) {
        self.shared.seam.notify(spawn.request());
        self.dismiss();
    }
}

fn moved(_: &mut Centre, (): ()) {}
