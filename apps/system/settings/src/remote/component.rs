use std::sync::Arc;

use rikka_app::prelude::*;
use rikka_capture::Screen;
use rikka_seam::net::SocketClient;
use rikka_seam::{Payload, SeamError};
use rikka_session::remote::{self, CAPABILITY, RemoteClient, RemoteStatus};
use rikka_settings::session::remote::{
    DISPLAY, DOMAIN, ENABLED, ENABLED_DEFAULT, MAX_BITRATE, MAX_BITRATE_DEFAULT, MAX_BITRATES,
    NETWORK, NETWORK_DEFAULT, PORT, PORT_DEFAULT, PORTS, SOUND, SOUND_DEFAULT, TAILSCALE,
    TAILSCALE_DEFAULT,
};
use rikka_widgets::column;
use serde::Serialize;

use crate::controls::{Count, flag_or, span};
use crate::domain::{Domain, domain_page};
use crate::notice::{acting_notice, domain_notice};
use crate::page::Page;
use crate::pending::Pending;
use crate::picker::value_picker;
use crate::remote::access::{Access, Opening};
use crate::remote::status;
use crate::session::Session;
use crate::settings::Settings;

const REACHING: &str = "remote-reaching";
const FOLLOWING: &str = "remote-following";
const WATCHING: &str = "remote-watching";
const LISTING: &str = "remote-listing";

const BITRATE_STEP: i64 = 5;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Output {
    name: String,
    width: u32,
    height: u32,
}

pub(crate) enum Link {
    Reaching,
    Unreachable(String),
    Following {
        connection: Arc<SocketClient>,
        status: Option<RemoteStatus>,
    },
    Lost,
}

pub(crate) struct Remote {
    domain: Domain,
    link: Link,
    outputs: Vec<Output>,
    failure: Option<String>,
}

impl Page for Remote {
    fn opened(&mut self, _: &Pending, session: &Session) {
        self.domain.open::<Self>(session);
        self.reach();
        spawn_blocking_keyed(
            LISTING,
            || {
                Screen::connect().map(|screen| {
                    screen
                        .outputs()
                        .into_iter()
                        .map(|output| Output {
                            name: output.name,
                            width: output.width,
                            height: output.height,
                        })
                        .collect::<Vec<_>>()
                })
            },
            |remote: &mut Remote, outputs: rikka_capture::Result<Vec<Output>>| {
                remote.outputs = outputs.unwrap_or_default();
            },
        );
    }

    fn closed(&mut self) {
        cancel(REACHING);
        cancel(FOLLOWING);
        cancel(WATCHING);
        cancel(LISTING);
        self.link = Link::Reaching;
        self.failure = None;
    }
}

domain_page!(Remote);

impl Remote {
    pub(crate) fn new() -> Self {
        Self {
            domain: Domain::new(DOMAIN),
            link: Link::Reaching,
            outputs: Vec::new(),
            failure: None,
        }
    }

    pub(crate) fn write(
        &mut self,
        pending: &mut Pending,
        key: &'static str,
        value: impl Serialize,
    ) {
        let before = self.opening();
        self.domain.set(key, value);
        before.follow(pending, self.opening());
    }

    pub(crate) fn status(&self) -> Option<&RemoteStatus> {
        match &self.link {
            Link::Following { status, .. } => status.as_ref(),
            Link::Reaching | Link::Unreachable(_) | Link::Lost => None,
        }
    }

    pub(crate) fn failure(&self) -> Option<&str> {
        self.failure.as_deref()
    }

    pub(crate) fn is_on(&self) -> bool {
        self.domain.get::<bool>(ENABLED).unwrap_or(ENABLED_DEFAULT)
    }

    pub(crate) fn opening(&self) -> Opening {
        Opening {
            port: self.domain.get::<i64>(PORT).unwrap_or(PORT_DEFAULT),
            network: self.domain.get::<bool>(NETWORK).unwrap_or(NETWORK_DEFAULT),
        }
    }

    pub(crate) fn reach(&mut self) {
        self.link = Link::Reaching;
        spawn_blocking_keyed(
            REACHING,
            || {
                SocketClient::for_capability(CAPABILITY)
                    .map(Arc::new)
                    .map_err(|error| error.to_string())
            },
            |remote: &mut Remote, reached: Result<Arc<SocketClient>, String>| {
                remote.reached(reached);
            },
        );
    }

    pub(crate) fn disconnect(&mut self) {
        self.call(RemoteClient::disconnect);
    }

    fn reached(&mut self, reached: Result<Arc<SocketClient>, String>) {
        let connection = match reached {
            Ok(connection) => connection,
            Err(reason) => {
                self.link = Link::Unreachable(reason);
                return;
            }
        };
        let seam = connection.client();
        spawn(
            RemoteClient::new(seam.clone()).status(),
            |remote: &mut Remote, reply: Result<Payload, SeamError>| remote.answered(reply),
        );
        stream_keyed(
            FOLLOWING,
            remote::statuses(&seam),
            |remote: &mut Remote, status: RemoteStatus| remote.show(status),
        );
        let watched = Arc::clone(&connection);
        spawn_keyed(
            WATCHING,
            async move { watched.closed().await },
            |remote: &mut Remote, (): ()| remote.lost(),
        );
        self.link = Link::Following {
            connection,
            status: None,
        };
    }

    fn answered(&mut self, reply: Result<Payload, SeamError>) {
        match reply.and_then(|payload| payload.decode::<RemoteStatus>()) {
            Ok(status) => self.show(status),
            Err(error) => self.failure = Some(error.to_string()),
        }
    }

    fn show(&mut self, shown: RemoteStatus) {
        if let Link::Following { status, .. } = &mut self.link {
            *status = Some(shown);
        }
    }

    fn lost(&mut self) {
        cancel(FOLLOWING);
        self.link = Link::Lost;
    }

    fn call(&mut self, method: fn(&RemoteClient) -> rikka_seam::Reply) {
        let Link::Following { connection, .. } = &self.link else {
            return;
        };
        self.failure = None;
        spawn(
            method(&RemoteClient::new(connection.client())),
            |remote: &mut Remote, reply: Result<Payload, SeamError>| {
                remote.failure = reply.err().map(|error| error.to_string());
            },
        );
    }
}

pub(crate) fn view(remote: &Remote, pending: &Pending) -> impl View {
    let on = remote.is_on();
    column! {
        domain_notice(
            &remote.domain,
            "settings.remote.unavailable",
            "settings.remote.refused",
        );
        link_notice(&remote.link);
        section(tr!("settings.remote.sharing"), sharing(remote));
        if on {
            status::addresses(remote);
        }
        if on {
            status::viewer(remote);
        }
        if on {
            section(tr!("settings.remote.connections"), connections(remote, pending));
        }
        if on {
            section(tr!("settings.remote.media"), media(remote));
        }
    }
    .gap(Space::ROOMY)
}

fn link_notice(link: &Link) -> Option<impl View> {
    let message = match link {
        Link::Reaching | Link::Following { .. } => return None,
        Link::Unreachable(reason) => tr!("settings.remote.service-unreachable", reason = reason),
        Link::Lost => tr!("settings.remote.service-stopped"),
    };
    Some(acting_notice(
        MaterialSymbol::CLOUD_OFF,
        palette().warning,
        message,
        tr!("settings.remote.reconnect"),
        |remote: &mut Remote| remote.reach(),
    ))
}

fn sharing(remote: &Remote) -> impl View {
    column! {
        flag_or(remote, tr!("settings.remote.enabled"), ENABLED, ENABLED_DEFAULT);
        if remote.is_on() {
            status::state(remote);
        }
    }
    .gap(Space::SNUG)
}

fn connections(remote: &Remote, pending: &Pending) -> impl View {
    let opening = remote.opening();
    column! {
        field(
            tr!("settings.remote.port"),
            number_field(opening.port as f32)
                .range(span(&PORTS))
                .on_commit(|settings: &mut Settings, port: f32| {
                    settings.write_remote(PORT, port.round() as i64);
                }),
        );
        field(
            tr!("settings.remote.network"),
            switch(opening.network).on_toggle(|settings: &mut Settings, on: bool| {
                settings.write_remote(NETWORK, on);
            }),
        );
        flag_or(remote, tr!("settings.remote.tailscale"), TAILSCALE, TAILSCALE_DEFAULT);
        firewall(opening, pending);
    }
    .gap(Space::SNUG)
}

fn firewall(opening: Opening, pending: &Pending) -> impl View {
    let access = opening.access(pending);
    let port = opening.port.to_string();
    column! {
        field(
            tr!("settings.remote.firewall"),
            switch(matches!(access, Access::Open | Access::Opening))
                .on_toggle(move |pending: &mut Pending, on: bool| opening.allow(pending, on))
                .enabled(access != Access::Unknown && !pending.busy()),
        );
        (access == Access::Closed).then(|| {
            text(tr!("settings.remote.firewall-closed", port = port.as_str()))
                .caption()
                .color(TextColor::Warning)
        });
    }
    .gap(Space::TIGHT)
}

fn media(remote: &Remote) -> impl View {
    let display = remote.domain.non_empty_string(DISPLAY);
    let outputs = remote
        .outputs
        .iter()
        .map(|output| {
            let label = tr!(
                "settings.remote.display-reading",
                name = output.name.as_str(),
                width = output.width,
                height = output.height
            );
            (output.name.clone(), label)
        })
        .collect();
    column! {
        Count {
            key: MAX_BITRATE,
            absent: MAX_BITRATE_DEFAULT,
            range: span(&MAX_BITRATES),
        }
        .field_in_steps(remote, tr!("settings.remote.max-bitrate"), BITRATE_STEP, |megabits| {
            tr!("settings.remote.megabits", count = megabits).to_string()
        });
        field(
            tr!("settings.remote.display"),
            value_picker(
                tr!("settings.remote.first-display"),
                outputs,
                display.as_deref(),
                |remote: &mut Remote, name: Option<String>| {
                    remote.domain.set(DISPLAY, name.unwrap_or_default());
                },
            ),
        );
        flag_or(remote, tr!("settings.remote.sound"), SOUND, SOUND_DEFAULT);
    }
    .gap(Space::SNUG)
}

impl std::fmt::Debug for Remote {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Remote")
            .field("on", &self.is_on())
            .finish_non_exhaustive()
    }
}
