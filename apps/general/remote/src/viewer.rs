use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rikka_app::prelude::*;
use rikka_remote_link::Pin;
use rikka_ui::Feed;
use rikka_widgets::{column, row};
use viewer_hosts::{Host, Hosts, Target};
use zeroize::Zeroizing;

use crate::computer;
use crate::live::{self, Live, Phase};
use crate::screen::Escape;
use crate::session::{self, Command, Link, Report};
use crate::tokens;

const FOLDER: &str = "remote-viewer";
const HOSTS: &str = "hosts.json";
const LIST: f32 = 520.0;

#[derive(State)]
pub(crate) struct Viewer {
    sender: Sender<Viewer>,
    hosts: Hosts,
    path: Option<PathBuf>,
    address: TextDocument,
    problem: Option<String>,
    live: Option<Live>,
    device: String,
    fullscreen: bool,
}

impl Viewer {
    pub(crate) fn new(sender: Sender<Viewer>) -> Viewer {
        let path = rikka_dirs::Paths::resolve()
            .ok()
            .map(|paths| paths.config_dir().join(FOLDER).join(HOSTS));
        let (hosts, problem) = match path.as_deref().map(Hosts::load) {
            Some(Ok(hosts)) => (hosts, None),
            Some(Err(error)) => (Hosts::default(), Some(error.to_string())),
            None => (Hosts::default(), None),
        };
        let (device, unnamed) = match computer::name() {
            Ok(name) => (name, None),
            Err(error) => (
                String::new(),
                Some(format!("failed to read the computer name: {error}")),
            ),
        };
        Viewer {
            sender,
            hosts,
            path,
            address: TextDocument::new(""),
            problem: problem.or(unnamed),
            live: None,
            device,
            fullscreen: false,
        }
    }

    pub(crate) fn set_fullscreen(&mut self, fullscreen: bool) {
        self.fullscreen = fullscreen;
    }

    fn add(&mut self) {
        let target = match Target::parse(&self.address.text()) {
            Ok(target) => target,
            Err(error) => {
                self.problem = Some(error.to_string());
                return;
            }
        };
        self.problem = None;
        self.address.reset("");
        let address = target.address();
        let host = self.hosts.get(&address).cloned().unwrap_or(Host {
            address,
            name: target.host,
            pin: None,
            used: now(),
        });
        self.open(host);
    }

    fn open(&mut self, host: Host) {
        match host.pin {
            Some(pin) => spawn(tokens::token(pin), move |viewer: &mut Viewer, token| {
                viewer.start(host.clone(), token);
            }),
            None => self.start(host, None),
        }
    }

    fn start(&mut self, host: Host, token: Option<Vec<u8>>) {
        let feed = Feed::new();
        let sender = self.sender.clone();
        let target = session::Target {
            address: host.address.clone(),
            pin: host.pin,
            token,
            device: self.device.clone(),
        };
        let reports = move |report: Report| {
            if sender
                .send(move |viewer: &mut Viewer| viewer.report(report))
                .is_err()
            {
                tracing::debug!("a report after the window closed");
            }
        };
        match Link::start(target, feed.clone(), reports) {
            Ok(link) => self.live = Some(Live::new(link, feed, host)),
            Err(error) => self.problem = Some(error.to_string()),
        }
    }

    fn report(&mut self, report: Report) {
        let Some(live) = &mut self.live else {
            return;
        };
        let seen = live.seen;
        let signed_in = match &report {
            Report::SignedIn { token } => {
                live.host.pin = seen;
                live.host.used = now();
                Some((live.host.clone(), token.clone()))
            }
            _ => None,
        };
        let refused = matches!(report, Report::TokenRefused);
        live.report(report);
        if let Some((host, token)) = signed_in {
            self.hosts.remember(host.clone());
            self.save();
            if let (Some(token), Some(pin)) = (token, seen) {
                spawn(
                    tokens::keep(pin, host.name, token),
                    |_: &mut Viewer, (): ()| {},
                );
            }
        }
        if let (true, Some(pin)) = (refused, seen) {
            spawn(tokens::forget(pin), |_: &mut Viewer, (): ()| {});
        }
    }

    pub(crate) fn send(&mut self, command: Command) {
        if let Some(live) = &self.live {
            live.send(command);
        }
    }

    pub(crate) fn sign_in(&mut self) {
        let Some(live) = &mut self.live else {
            return;
        };
        if live.password.is_empty() {
            return;
        }
        let password = Zeroizing::new(live.password.text());
        live.password.reset("");
        live.phase = Phase::Connecting;
        live.send(Command::SignIn {
            password,
            remember: live.remember,
        });
    }

    pub(crate) fn set_remember(&mut self, on: bool) {
        if let Some(live) = &mut self.live {
            live.remember = on;
        }
    }

    pub(crate) fn trust(&mut self, seen: Pin) {
        let Some(live) = self.live.take() else {
            return;
        };
        let mut host = live.host.clone();
        drop(live);
        if let Some(previous) = host.pin {
            spawn(tokens::forget(previous), |_: &mut Viewer, (): ()| {});
        }
        host.pin = Some(seen);
        self.hosts.remember(host.clone());
        self.save();
        self.start(host, None);
    }

    pub(crate) fn reconnect(&mut self) {
        if let Some(live) = self.live.take() {
            let host = self
                .hosts
                .get(&live.host.address)
                .cloned()
                .unwrap_or(live.host.clone());
            drop(live);
            self.open(host);
        }
    }

    pub(crate) fn leave(&mut self) {
        self.live = None;
        if self.fullscreen {
            request_window(WindowRequest::Fullscreen(false));
        }
    }

    pub(crate) fn escape(&mut self, escape: Escape) {
        match escape {
            Escape::Leave => self.leave(),
            Escape::Fullscreen => request_window(WindowRequest::ToggleFullscreen),
            Escape::Stats => {
                if let Some(live) = &mut self.live {
                    live.toggle_stats();
                }
            }
        }
    }

    fn forget(&mut self, address: &str) {
        if let Some(host) = self.hosts.forget(address) {
            if let Some(pin) = host.pin {
                spawn(tokens::forget(pin), |_: &mut Viewer, (): ()| {});
            }
            self.save();
        }
    }

    fn save(&mut self) {
        let Some(path) = &self.path else {
            return;
        };
        if let Err(error) = self.hosts.save(path) {
            self.problem = Some(error.to_string());
        }
    }
}

impl std::fmt::Debug for Viewer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Viewer")
            .field("live", &self.live)
            .finish_non_exhaustive()
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

pub(crate) fn view(viewer: &Viewer) -> impl View {
    match &viewer.live {
        Some(live) => live::view(live, viewer.fullscreen).any(),
        None => hosts(viewer).any(),
    }
}

fn hosts(viewer: &Viewer) -> impl View {
    let listed = viewer.hosts.list();
    let list = column! {
        text(tr!("remote.hosts.heading")).title();
        row! {
            text_field(&viewer.address)
                .placeholder(tr!("remote.hosts.placeholder"))
                .autofocus(true)
                .on_change(|| {})
                .on_submit(Viewer::add)
                .grow(1.0);
            button(tr!("remote.hosts.connect"))
                .primary()
                .enabled(!viewer.address.is_empty())
                .on_press(Viewer::add);
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width();
        viewer
            .problem
            .as_ref()
            .map(|problem| text(problem.clone()).caption().color(TextColor::Danger));
        if listed.is_empty() {
            text(tr!("remote.hosts.empty")).label().muted();
        }
        for host in listed {
            known(host);
        }
    }
    .gap(Space::NORMAL)
    .width(LIST);
    scrollable(
        column! {
            list;
        }
        .align_items(Align::Center)
        .padding(Space::WIDE)
        .fill_width(),
    )
    .fill()
}

fn known(host: &Host) -> impl View {
    let opened = host.clone();
    let forgotten = host.address.clone();
    let detail = match host.pin {
        Some(pin) => tr!(
            "remote.hosts.fingerprint",
            fingerprint = pin.to_string().as_str()
        ),
        None => tr!("remote.hosts.new"),
    };
    row! {
        icon(MaterialSymbol::CONNECTED_TV).color(palette().text_muted);
        column! {
            text(host.name.clone()).label().medium();
            text(format!("{} · {}", host.address, detail)).caption().muted();
        }
        .gap(Space::HAIR)
        .grow(1.0);
        button(tr!("remote.hosts.forget"))
            .ghost()
            .small()
            .on_press(move |viewer: &mut Viewer| viewer.forget(&forgotten));
        button(tr!("remote.hosts.connect"))
            .secondary()
            .small()
            .on_press(move |viewer: &mut Viewer| viewer.open(opened.clone()));
    }
    .gap(Space::NORMAL)
    .align_items(Align::Center)
    .fill_width()
}
