use std::sync::mpsc::Receiver;

use rikka_clipboard::Clipboard;
use rikka_seam::{Call, CapabilityId, Delivery, Request, Seam, SeamClient, Service};
use rikka_session::shortcuts::{self, Shortcut};
use rikka_settings::Schema;
use rikka_settings::seam::Settings;
use rikka_theme::Theme;
use rikka_ui::State;
use serde::de::DeserializeOwned;

use crate::domains::{Domains, Reader};
use crate::erased::ErasedSurface;
use crate::error::{Error, Result};
use crate::host::{Host, Slot};
use crate::idle::Idles;
use crate::parts::Parts;
use crate::seam_sender::SeamSender;
use crate::session::Session;
use crate::shared_theme::SharedTheme;
use crate::surface::SurfaceDesc;
use crate::waker::LoopWaker;
use crate::wall::WallClock;
use crate::{Aimed, Idle, KeyboardState, Outputs, Typist};

type Act = Box<dyn Fn(&mut Host, &Delivery)>;

pub struct Watcher {
    pub capability: CapabilityId,
    pub act: Act,
}

impl std::fmt::Debug for Watcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Watcher")
            .field("capability", &self.capability)
            .finish_non_exhaustive()
    }
}

pub struct Shell {
    name: String,
    shared_theme: SharedTheme,
    surfaces: Vec<ErasedSurface>,
    watchers: Vec<Watcher>,
    seam: Seam<Host>,
    waker: LoopWaker,
    calls: Receiver<Call>,
    session: Option<Session>,
    domains: Domains,
    wall: WallClock,
    idles: Idles,
    keyboard: KeyboardState,
    host: Host,
    error: Option<Error>,
}

impl Shell {
    pub fn new(name: impl Into<String>) -> Self {
        let waker = LoopWaker::default();
        let (sender, calls) = waker.channel::<Call>();
        let (seam, session) = Session::connect(Seam::new(SeamSender::new(sender)));
        let host = Host::new(seam.client());
        Self {
            name: name.into(),
            shared_theme: SharedTheme::default(),
            surfaces: Vec::new(),
            watchers: Vec::new(),
            seam,
            waker,
            calls,
            session,
            domains: Domains::default(),
            wall: WallClock::new(),
            idles: Idles::default(),
            keyboard: KeyboardState::default(),
            host,
            error: None,
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn wall(&self) -> WallClock {
        self.wall.clone()
    }

    #[must_use]
    pub fn idle(&self) -> Idle {
        self.idles.add()
    }

    #[must_use]
    pub fn keyboard(&self) -> KeyboardState {
        self.keyboard.clone()
    }

    #[must_use]
    pub fn clipboard(&self) -> Option<Clipboard> {
        self.host.clipboard()
    }

    #[must_use]
    pub fn typist(&self) -> Typist {
        self.host.typist().clone()
    }

    #[must_use]
    pub fn outputs(&self) -> Outputs {
        self.host.outputs().clone()
    }

    #[must_use]
    pub fn hold<T: 'static>(mut self, value: T) -> (Shell, Slot<T>) {
        let slot = self.host.hold(value);
        (self, slot)
    }

    pub fn theme(self, theme: Theme) -> Self {
        self.shared_theme.set(theme);
        self
    }

    pub fn shared_theme(&self) -> SharedTheme {
        self.shared_theme.clone()
    }

    pub fn surface<S: State>(mut self, desc: SurfaceDesc<S>) -> Self {
        self.surfaces.push(ErasedSurface::new(desc));
        self
    }

    #[must_use]
    pub fn feature(self, feature: impl crate::Feature) -> Shell {
        feature.install(self)
    }

    pub fn service(mut self, service: impl Service<Host>) -> Self {
        if let Err(error) = self.seam.register(service) {
            self.error.get_or_insert(Error::Registration(error));
        }
        self
    }

    pub fn watch(
        mut self,
        capability: impl Into<CapabilityId>,
        act: impl Fn(&mut Host, &Delivery) + 'static,
    ) -> Self {
        self.watchers.push(Watcher {
            capability: capability.into(),
            act: Box::new(act),
        });
        self
    }

    pub fn on_shortcut(self, shortcut: Shortcut, request: Request) -> Self {
        self.on_shortcuts(move |pressed| (pressed == shortcut).then(|| request.clone()))
    }

    pub fn on_shortcuts(self, request_for: impl Fn(Shortcut) -> Option<Request> + 'static) -> Self {
        self.watch(shortcuts::CAPABILITY, move |host, incoming| {
            if let Some(request) = incoming
                .event()
                .and_then(shortcuts::activated)
                .and_then(&request_for)
            {
                host.seam().notify(request);
            }
        })
    }

    pub fn settings(mut self, schema: Schema) -> Self {
        let session = self.session.as_ref().and_then(Session::client);
        self.domains.declare(schema, session);
        self
    }

    pub fn setting<T: DeserializeOwned>(&self, domain: &str, key: &str) -> Option<T> {
        self.domains.get(domain, key)
    }

    pub fn reader(&self, domain: &str) -> Option<Reader> {
        self.domains.reader(domain)
    }

    pub fn follow<T: DeserializeOwned>(
        mut self,
        domain: &'static str,
        key: &'static str,
        act: impl Fn(&mut Host, T) + 'static,
    ) -> Self {
        if let Some(value) = self.setting(domain, key) {
            act(&mut self.host, value);
        }
        self.on_setting(domain, key, act)
    }

    pub fn on_setting<T: DeserializeOwned>(
        mut self,
        domain: &'static str,
        key: &'static str,
        act: impl Fn(&mut Host, T) + 'static,
    ) -> Self {
        self.domains.follow(
            domain,
            Some(key),
            Box::new(move |host: &mut Host, settings: &Settings| {
                if let Some(value) = settings.read(key) {
                    act(host, value);
                }
            }),
        );
        self
    }

    pub fn follow_domain(
        mut self,
        domain: &'static str,
        act: impl Fn(&mut Host, &Settings) + 'static,
    ) -> Self {
        if let Some(reader) = self.reader(domain) {
            act(&mut self.host, &reader.borrow());
        }
        self.on_domain(domain, act)
    }

    pub fn on_domain(
        mut self,
        domain: &'static str,
        act: impl Fn(&mut Host, &Settings) + 'static,
    ) -> Self {
        self.domains.follow(domain, None, Box::new(act));
        self
    }

    pub fn seam(&self) -> SeamClient {
        self.seam.client()
    }

    #[must_use]
    pub fn aimed(&self) -> Aimed {
        Aimed::new(self.session.as_ref().and_then(Session::client))
    }

    pub fn into_parts(self) -> Result<Parts> {
        if let Some(error) = self.error {
            return Err(error);
        }
        for desc in &self.surfaces {
            desc.settings.validate()?;
        }
        Ok(Parts {
            name: self.name,
            shared_theme: self.shared_theme,
            surfaces: self.surfaces,
            watchers: self.watchers,
            seam: self.seam,
            waker: self.waker,
            calls: self.calls,
            session: self.session,
            domains: self.domains,
            wall: self.wall,
            idles: self.idles,
            keyboard: self.keyboard,
            host: self.host,
        })
    }
}

impl std::fmt::Debug for Shell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shell")
            .field("name", &self.name)
            .field("skin", &self.shared_theme)
            .field("surfaces", &self.surfaces)
            .field("watching", &self.watchers.len())
            .field("seam", &self.seam)
            .field("session", &self.session)
            .field("domains", &self.domains)
            .field("host", &self.host)
            .finish_non_exhaustive()
    }
}
