use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use rikka_bluetooth::{
    Action, Adapter, AdapterId, Device, DeviceId, Event, Prompt, Question, Reply, Request, Snapshot,
};

use crate::bluetooth::{detail, devices, pairing};
use crate::notice::{error_notice, notice};
use crate::page::Page;
use crate::pending::Pending;
use crate::picker::picker;
use crate::session::Session;

const WIDE: f32 = 260.0;
const FOLLOWING: &str = "settings.bluetooth.following";
const HEARING: &str = "settings.bluetooth.hearing";
const REACHING: &str = "settings.bluetooth.reaching";

pub(crate) struct Bluetooth {
    daemon: Option<rikka_bluetooth::Bluetooth>,
    unavailable: Option<String>,
    snapshot: Snapshot,
    adapter: Option<AdapterId>,
    opened: Option<DeviceId>,
    prompt: Option<Prompt>,
    typed: TextDocument,
    name: TextDocument,
    label: TextDocument,
    busy: Option<(Action, Option<DeviceId>)>,
    error: Option<String>,
    looking: bool,
}

impl Page for Bluetooth {
    fn opened(&mut self, _: &Pending, _: &Session) {
        if self.daemon.is_none() {
            self.connect();
        }
        self.looking = true;
        self.error = None;
        self.resume();
    }

    fn closed(&mut self) {
        self.looking = false;
        let (Some(daemon), Some(adapter)) = (&self.daemon, self.adapter()) else {
            return;
        };
        if adapter.discovering {
            daemon.send(Request::Scan(adapter.id.clone(), false));
        }
    }
}

impl Bluetooth {
    pub(crate) fn new() -> Self {
        Self {
            daemon: None,
            unavailable: None,
            snapshot: Snapshot::default(),
            adapter: None,
            opened: None,
            prompt: None,
            typed: TextDocument::new(""),
            name: TextDocument::new(""),
            label: TextDocument::new(""),
            busy: None,
            error: None,
            looking: false,
        }
    }

    fn connect(&mut self) {
        let daemon = rikka_bluetooth::Bluetooth::answering();
        stream_keyed(
            FOLLOWING,
            daemon.changes(),
            |bluetooth: &mut Bluetooth, state: rikka_bluetooth::State| {
                bluetooth.follow(state.snapshot);
            },
        );
        stream_keyed(
            HEARING,
            daemon.events(),
            |bluetooth: &mut Bluetooth, event: Event| bluetooth.report(event),
        );
        stream_keyed(
            REACHING,
            daemon.unavailable_changes(),
            |bluetooth: &mut Bluetooth, reason: Option<String>| bluetooth.unavailable = reason,
        );
        self.unavailable = daemon.unavailable();
        self.snapshot = daemon.state().snapshot;
        self.daemon = Some(daemon);
    }

    fn resume(&mut self) {
        if !self.looking || self.error.is_some() {
            return;
        }
        if self
            .busy
            .as_ref()
            .is_some_and(|(action, _)| *action == Action::Scan)
        {
            return;
        }
        let Some(adapter) = self.adapter().filter(|adapter| adapter.powered) else {
            return;
        };
        if adapter.discovering {
            return;
        }
        let id = adapter.id.clone();
        self.send(Request::Scan(id, true));
    }

    pub(super) fn send(&mut self, request: Request) {
        let Some(daemon) = &self.daemon else {
            return;
        };
        self.busy = Some((request.action(), request.device().cloned()));
        self.error = None;
        daemon.send(request);
    }

    fn adapter(&self) -> Option<&Adapter> {
        match &self.adapter {
            Some(id) => self
                .snapshot
                .adapter(id)
                .or_else(|| self.snapshot.adapters.first()),
            None => self.snapshot.adapters.first(),
        }
    }

    fn adapter_id(&self) -> Option<AdapterId> {
        self.adapter().map(|adapter| adapter.id.clone())
    }

    fn opened_device(&self) -> Option<&Device> {
        self.snapshot.device(self.opened.as_ref()?)
    }

    pub(super) fn choose_adapter(&mut self, index: usize) {
        self.adapter = self
            .snapshot
            .adapters
            .get(index)
            .map(|adapter| adapter.id.clone());
    }

    pub(super) fn power(&mut self, on: bool) {
        if let Some(id) = self.adapter_id() {
            self.send(Request::Power(id.clone(), on));
            if on {
                self.send(Request::Scan(id, true));
            }
        }
    }

    pub(super) fn discoverable(&mut self, on: bool) {
        if let Some(id) = self.adapter_id() {
            self.send(Request::Discoverable(id, on));
        }
    }

    pub(super) fn rename(&mut self) {
        let name = self.name.text();
        if let Some(adapter) = self.adapter()
            && adapter.alias != name
            && !name.is_empty()
        {
            let id = adapter.id.clone();
            self.send(Request::Rename(id, name));
        }
    }

    pub(super) fn scan(&mut self, on: bool) {
        self.looking = on;
        if let Some(id) = self.adapter_id() {
            self.send(Request::Scan(id, on));
        }
    }

    pub(super) fn forget(&mut self, id: DeviceId) {
        if self.opened.as_ref() == Some(&id) {
            self.opened = None;
        }
        self.send(Request::Forget(id));
    }

    pub(super) fn inspect(&mut self, id: DeviceId) {
        let title = self
            .snapshot
            .device(&id)
            .map(|device| device.title.clone())
            .unwrap_or_default();
        self.label.set_text(title);
        self.opened = Some(id);
    }

    pub(super) fn relabel(&mut self) {
        let label = self.label.text();
        if let Some(device) = self.opened_device()
            && device.title != label
            && !label.is_empty()
        {
            let id = device.id.clone();
            self.send(Request::Label(id, label));
        }
    }

    fn follow(&mut self, snapshot: Snapshot) {
        self.snapshot = snapshot;
        if let Some(adapter) = self.adapter()
            && self.name.text() != adapter.alias
            && self.name.is_empty()
        {
            let alias = adapter.alias.clone();
            self.name.set_text(alias);
        }
        self.resume();
    }

    fn report(&mut self, event: Event) {
        match event {
            Event::Changed(_) => {}
            Event::Answered { action, error, .. } => {
                if self.busy.as_ref().map(|(busy, _)| *busy) == Some(action) {
                    self.busy = None;
                }
                self.error = error;
                self.resume();
            }
            Event::Asked(prompt) => {
                self.typed.clear();
                self.prompt = Some(prompt);
            }
            Event::Withdrew(id) => {
                if self.prompt.as_ref().map(|prompt| prompt.id) == Some(id) {
                    self.prompt = None;
                }
            }
        }
    }

    pub(super) fn answer(&mut self, yes: bool) {
        let (Some(daemon), Some(prompt)) = (&self.daemon, self.prompt.take()) else {
            return;
        };
        let reply = match (yes, &prompt.question) {
            (false, _) => Reply::No,
            (true, Question::AskPasskey) => match self.typed.text().parse::<u32>() {
                Ok(passkey) => Reply::Passkey(passkey),
                Err(_) => Reply::No,
            },
            (true, Question::AskPin) => Reply::Pin(self.typed.text()),
            (true, _) => Reply::Yes,
        };
        daemon.answer(prompt.id, reply);
    }
}

pub(crate) fn view(bluetooth: &Bluetooth) -> Element {
    if let Some(reason) = &bluetooth.unavailable {
        return notice(
            MaterialSymbol::BLUETOOTH_DISABLED,
            palette().warning,
            tr!("settings.bluetooth.unavailable", reason = reason.as_str()),
        )
        .build();
    }
    let Some(adapter) = bluetooth.adapter() else {
        return notice(
            MaterialSymbol::BLUETOOTH_DISABLED,
            palette().text_muted,
            tr!("settings.bluetooth.no-adapter"),
        )
        .build();
    };

    page_stack(
        usize::from(bluetooth.opened_device().is_some()),
        body(bluetooth, adapter),
    )
    .fill_width()
    .build()
}

fn body(bluetooth: &Bluetooth, adapter: &Adapter) -> impl View {
    let waiting = bluetooth
        .busy
        .as_ref()
        .and_then(|(_, device)| device.as_ref());
    let pairing = bluetooth.busy.as_ref().map(|(action, _)| *action);
    column! {
        if adapter.powered
            && let Some(device) = bluetooth.opened_device()
        {
            header(device);
            detail::view(device, &bluetooth.label);
        } else {
            radio(bluetooth, adapter);
            error_notice(
                None,
                bluetooth.error.as_deref(),
                (
                    "settings.bluetooth.unavailable",
                    "settings.bluetooth.refused",
                ),
            );
            bluetooth
                .prompt
                .as_ref()
                .map(|prompt| pairing::view(prompt, &bluetooth.typed, &bluetooth.snapshot));
            if adapter.powered {
                devices::known(&bluetooth.snapshot, &adapter.id, waiting);
                devices::nearby(
                    &bluetooth.snapshot,
                    &adapter.id,
                    adapter.discovering,
                    waiting,
                    pairing,
                );
            }
        }
    }
    .gap(Space::ROOMY)
    .fill_width()
}

fn radio(bluetooth: &Bluetooth, adapter: &Adapter) -> impl View {
    section(
        tr!("settings.bluetooth.title"),
        column! {
            field(
                tr!("settings.bluetooth.enabled"),
                switch(adapter.powered)
                    .on_toggle(|bluetooth: &mut Bluetooth, on: bool| bluetooth.power(on)),
            );
            if bluetooth.snapshot.adapters.len() > 1 {
                field(
                    tr!("settings.bluetooth.adapter"),
                    picker(
                        bluetooth
                            .snapshot
                            .adapters
                            .iter()
                            .position(|listed| listed.id == adapter.id),
                        bluetooth
                            .snapshot
                            .adapters
                            .iter()
                            .map(|listed| Label::from(listed.alias.as_str())),
                        |bluetooth: &mut Bluetooth, index: usize| bluetooth.choose_adapter(index),
                    ),
                );
            }
            if adapter.powered {
                field(
                    tr!("settings.bluetooth.name"),
                    text_field(&bluetooth.name)
                        .on_change(|| {})
                        .on_submit(|bluetooth: &mut Bluetooth| bluetooth.rename())
                        .on_blur(|bluetooth: &mut Bluetooth| bluetooth.rename())
                        .width(WIDE),
                );
                field(
                    tr!("settings.bluetooth.discoverable"),
                    switch(adapter.discoverable).on_toggle(|bluetooth: &mut Bluetooth, on: bool| {
                        bluetooth.discoverable(on);
                    }),
                );
            }
        }
        .gap(Space::SNUG)
        .fill_width(),
    )
}

fn header(device: &Device) -> impl View {
    row! {
        button(icon(MaterialSymbol::ARROW_BACK))
            .ghost()
            .icon_only()
            .on_press(|bluetooth: &mut Bluetooth| bluetooth.opened = None);
        text(device.title.as_str()).heading().semibold();
        spacer();
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
}

impl std::fmt::Debug for Bluetooth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bluetooth").finish_non_exhaustive()
    }
}
