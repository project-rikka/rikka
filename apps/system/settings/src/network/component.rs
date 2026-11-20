use std::time::Duration;

use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use rikka_network::{Action, ActiveId, Device, DeviceId, DeviceKind, Event, Request, Snapshot};

use crate::network::detail::{self, Detail};
use crate::network::hotspot::{self, HotspotForm};
use crate::network::join::{self, JoinForm};
use crate::network::wireless;
use crate::notice::{error_notice, notice};
use crate::page::Page;
use crate::pending::Pending;
use crate::session::Session;

const LOOKING: Duration = Duration::from_secs(8);
const FOLLOWING: &str = "settings.network.following";
const HEARING: &str = "settings.network.hearing";
const REACHING: &str = "settings.network.reaching";

#[derive(State)]
enum Screen {
    List,
    Joining(#[state] Box<JoinForm>),
    Inspecting(#[state] Box<Detail>),
    Sharing(#[state] Box<HotspotForm>),
}

#[derive(State)]
pub(crate) struct Network {
    daemon: Option<rikka_network::Network>,
    unavailable: Option<String>,
    snapshot: Snapshot,
    radio: Option<DeviceId>,
    #[state]
    screen: Screen,
    busy: Option<Action>,
    looking: bool,
    scanned: bool,
    error: Option<String>,
}

impl Page for Network {
    fn opened(&mut self, _: &Pending, _: &Session) {
        if self.daemon.is_some() {
            return;
        }
        let daemon = rikka_network::Network::start();
        stream_keyed(
            FOLLOWING,
            daemon.changes(),
            |network: &mut Network, state: rikka_network::State| network.follow(state.snapshot),
        );
        stream_keyed(
            HEARING,
            daemon.events(),
            |network: &mut Network, event: Event| network.report(event),
        );
        stream_keyed(
            REACHING,
            daemon.unavailable_changes(),
            |network: &mut Network, reason: Option<String>| network.unavailable = reason,
        );
        self.unavailable = daemon.unavailable();
        let snapshot = daemon.state().snapshot;
        self.daemon = Some(daemon);
        self.follow(snapshot);
    }

    fn closed(&mut self) {
        self.scanned = false;
    }
}

impl Network {
    pub(crate) fn new() -> Self {
        Self {
            daemon: None,
            unavailable: None,
            snapshot: Snapshot::default(),
            radio: None,
            screen: Screen::List,
            busy: None,
            looking: false,
            scanned: false,
            error: None,
        }
    }

    fn follow(&mut self, snapshot: Snapshot) {
        self.snapshot = snapshot;
        if !self.scanned && self.radio().is_some() {
            self.scanned = true;
            self.scan();
        }
    }

    fn report(&mut self, event: Event) {
        match event {
            Event::Changed(_) => skip_view(),
            Event::Answered { action, error } => {
                if self.busy == Some(action) {
                    self.busy = None;
                }
                self.error = error;
                if self.error.is_none()
                    && matches!(action, Action::Join | Action::Hotspot | Action::Save)
                {
                    self.screen = Screen::List;
                }
            }
            Event::Revealed { uuid, secrets } => {
                if let Screen::Inspecting(detail) = &mut self.screen
                    && detail.uuid == uuid
                {
                    detail.reveal(&secrets);
                }
            }
        }
    }

    pub(super) fn send(&mut self, request: Request) {
        let Some(daemon) = &self.daemon else {
            return;
        };
        self.busy = Some(request.action());
        self.error = None;
        daemon.send(request);
    }

    fn devices(&self, kind: DeviceKind) -> Vec<&Device> {
        self.snapshot.devices_of(kind).collect()
    }

    fn radio(&self) -> Option<&Device> {
        let radios = self.devices(DeviceKind::Wireless);
        match &self.radio {
            Some(id) => radios
                .iter()
                .find(|radio| &radio.id == id)
                .or_else(|| radios.first())
                .copied(),
            None => radios.first().copied(),
        }
    }

    pub(super) fn choose_radio(&mut self, index: usize) {
        self.radio = self
            .devices(DeviceKind::Wireless)
            .get(index)
            .map(|radio| radio.id.clone());
    }

    pub(super) fn scan(&mut self) {
        let Some(device) = self.radio().map(|radio| radio.id.clone()) else {
            return;
        };
        self.send(Request::Scan(device));
        self.looking = true;
        after(LOOKING, |network: &mut Network| network.looking = false);
    }

    pub(super) fn back(&mut self) {
        self.screen = Screen::List;
        self.error = None;
    }

    pub(crate) fn go_back(&mut self) -> bool {
        if matches!(self.screen, Screen::List) {
            return false;
        }
        self.back();
        true
    }

    pub(super) fn start(&mut self, uuid: String) {
        let device = self.device_for(&uuid);
        self.send(Request::Activate { uuid, device });
    }

    pub(super) fn stop(&mut self, active: ActiveId) {
        self.send(Request::Deactivate(active));
    }

    pub(super) fn inspect(&mut self, uuid: &str) {
        if let Some(profile) = self.snapshot.profile(uuid) {
            self.screen = Screen::Inspecting(Box::new(Detail::new(profile)));
        }
    }

    pub(super) fn forget(&mut self, uuid: String) {
        if matches!(&self.screen, Screen::Inspecting(detail) if detail.uuid == uuid) {
            self.screen = Screen::List;
        }
        self.send(Request::Forget(uuid));
    }

    pub(super) fn hidden(&mut self) {
        if let Some(device) = self.radio().map(|radio| radio.id.clone()) {
            self.screen = Screen::Joining(Box::new(JoinForm::hidden(device)));
        }
    }

    pub(super) fn hotspot(&mut self) {
        if let Some(device) = self.radio().map(|radio| radio.id.clone()) {
            let hostname = self.snapshot.hostname.clone();
            self.screen = Screen::Sharing(Box::new(HotspotForm::new(device, &hostname)));
        }
    }

    fn reveal(&mut self) {
        if let Screen::Inspecting(detail) = &self.screen {
            let uuid = detail.uuid.clone();
            self.send(Request::Reveal(uuid));
        }
    }

    fn apply(&mut self) {
        if let Screen::Inspecting(detail) = &self.screen {
            let profile = detail.profile();
            self.send(Request::Save(Box::new(profile)));
        }
    }

    fn device_for(&self, uuid: &str) -> Option<DeviceId> {
        let profile = self.snapshot.profile(uuid)?;
        profile.wireless.as_ref()?;
        self.radio().map(|radio| radio.id.clone())
    }

    pub(super) fn join(&mut self, index: usize) {
        let Some(device) = self.radio().map(|radio| radio.id.clone()) else {
            return;
        };
        let Some(point) = self
            .snapshot
            .networks(&device)
            .get(index)
            .map(|point| (*point).clone())
        else {
            return;
        };
        if point.security.is_open() {
            let form = JoinForm::visible(device, &point);
            self.send(Request::Join(Box::new(form.request())));
            return;
        }
        self.screen = Screen::Joining(Box::new(JoinForm::visible(device, &point)));
    }

    fn submit(&mut self) {
        let request = match &self.screen {
            Screen::Joining(form) if form.is_ready() => Request::Join(Box::new(form.request())),
            Screen::Sharing(form) if form.is_ready() => Request::Hotspot(Box::new(form.request())),
            _ => return,
        };
        self.send(request);
    }
}

pub(crate) fn view(network: &Network) -> impl View {
    let deeper = usize::from(!matches!(network.screen, Screen::List));
    column! {
        match unreachable(network) {
            Some(unreachable) => unreachable,
            None => column! {
                error_notice(
                    None,
                    network.error.as_deref(),
                    ("settings.network.unavailable", "settings.network.refused"),
                );
                page_stack(deeper, body(network)).fill_width();
            }
            .gap(Space::ROOMY)
            .fill_width()
            .build(),
        }
    }
    .gap(Space::ROOMY)
    .fill_width()
}

fn unreachable(network: &Network) -> Option<Element> {
    let reason = network.unavailable.as_ref()?;
    Some(
        notice(
            MaterialSymbol::CLOUD_OFF,
            palette().warning,
            tr!("settings.network.unavailable", reason = reason.as_str()),
        )
        .build(),
    )
}

fn body(network: &Network) -> Element {
    match &network.screen {
        Screen::Joining(form) => joining(network, form).build(),
        Screen::Inspecting(detail) => inspecting(network, detail).build(),
        Screen::Sharing(form) => sharing(network, form).build(),
        Screen::List => wireless::view(
            &network.snapshot,
            &network.devices(DeviceKind::Wireless),
            network.radio(),
            network.busy,
            network.looking,
        )
        .build(),
    }
}

fn joining(network: &Network, form: &JoinForm) -> impl View {
    let title = form.title();
    column! {
        header(
            match title.is_empty() {
                true => tr!("settings.network.join.hidden"),
                false => Label::from(title),
            },
            row! {
                button(text(tr!("settings.network.cancel")))
                    .secondary()
                    .on_press(|network: &mut Network| network.back());
                button(text(tr!("settings.network.join.connect")))
                    .primary()
                    .enabled(form.is_ready())
                    .loading(network.busy == Some(Action::Join))
                    .on_press(|network: &mut Network| network.submit());
            }
            .gap(Space::SNUG)
            .align_items(Align::Center),
        );
        card(join::view(form));
    }
    .gap(Space::ROOMY)
    .fill_width()
}

fn sharing(network: &Network, form: &HotspotForm) -> impl View {
    column! {
        header(
            tr!("settings.network.hotspot.title"),
            row! {
                button(text(tr!("settings.network.cancel")))
                    .secondary()
                    .on_press(|network: &mut Network| network.back());
                button(text(tr!("settings.network.hotspot.start")))
                    .primary()
                    .enabled(form.is_ready())
                    .loading(network.busy == Some(Action::Hotspot))
                    .on_press(|network: &mut Network| network.submit());
            }
            .gap(Space::SNUG)
            .align_items(Align::Center),
        );
        card(hotspot::view(form));
    }
    .gap(Space::ROOMY)
    .fill_width()
}

fn inspecting(network: &Network, detail: &Detail) -> impl View {
    let uuid = detail.uuid.clone();
    let actions = row! {
        if detail.has_secret() {
            button(text(tr!("settings.network.detail.reveal")))
                .secondary()
                .loading(network.busy == Some(Action::Reveal))
                .on_press(|network: &mut Network| network.reveal());
        }
        button(text(tr!("settings.network.detail.forget")))
            .danger()
            .on_press(move |network: &mut Network| network.forget(uuid.clone()));
        button(text(tr!("settings.network.detail.apply")))
            .primary()
            .enabled(detail.is_edited())
            .loading(network.busy == Some(Action::Save))
            .on_press(|network: &mut Network| network.apply());
    }
    .gap(Space::SNUG)
    .align_items(Align::Center);

    column! {
        header(Label::from(detail.title()), actions);
        detail::view(detail, &network.snapshot);
    }
    .gap(Space::ROOMY)
    .fill_width()
}

fn header(title: Label, actions: impl View) -> impl View {
    row! {
        button(icon(MaterialSymbol::ARROW_BACK))
            .ghost()
            .icon_only()
            .on_press(|network: &mut Network| network.back());
        text(title).heading().semibold();
        spacer();
        actions;
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
}

impl std::fmt::Debug for Network {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Network").finish_non_exhaustive()
    }
}
