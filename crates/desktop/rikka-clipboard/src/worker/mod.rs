mod content;
mod control;
mod state;

use std::ops::ControlFlow;
use std::os::unix::net::UnixStream;

use rikka_wayland::{EventLoop, Host};
use rikka_wayland_client::Route;
use rikka_wayland_client::ext_data_control_device_v1::ExtDataControlDeviceV1;
use rikka_wayland_client::ext_data_control_offer_v1::ExtDataControlOfferV1;
use rikka_wayland_client::ext_data_control_source_v1::ExtDataControlSourceV1;
use rikka_wayland_client::wl_data_device::WlDataDevice;
use rikka_wayland_client::wl_data_device_manager::WlDataDeviceManager;
use rikka_wayland_client::wl_data_offer::WlDataOffer;
use rikka_wayland_client::wl_data_source::WlDataSource;
use rikka_wayland_client::wl_seat::WlSeat;
use rikka_wayland_client::zwp_primary_selection_device_manager_v1::ZwpPrimarySelectionDeviceManagerV1;
use rikka_wayland_client::zwp_primary_selection_device_v1::ZwpPrimarySelectionDeviceV1;
use rikka_wayland_client::zwp_primary_selection_offer_v1::ZwpPrimarySelectionOfferV1;
use rikka_wayland_client::zwp_primary_selection_source_v1::ZwpPrimarySelectionSourceV1;
use rikka_worker::Reply;

use rikka_clipboard_core::{Error, Result, Selection};

use crate::error::{self, startup};
use crate::watch::Watcher;

pub(crate) use content::Content;
use control::{
    Control, on_ext_data_control_device_v1, on_ext_data_control_offer_v1,
    on_ext_data_control_source_v1,
};
use state::{
    Published, State, ensure_offered, on_wl_data_device, on_wl_data_offer, on_wl_data_source,
    on_zwp_primary_selection_device_v1, on_zwp_primary_selection_offer_v1,
    on_zwp_primary_selection_source_v1, transfer,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Link {
    Owned,
    Shared,
}

pub(crate) type CommandSender = rikka_worker::CommandSender<Command>;
type CommandReceiver = rikka_worker::CommandReceiver<Command>;

pub(crate) enum Command {
    Request(Request),
    Stop,
}

pub(crate) enum Request {
    Publish {
        content: Content,
        selection: Selection,
        by: u64,
    },
    Withdraw {
        selection: Selection,
        by: u64,
    },
    MimeTypes {
        selection: Selection,
        reply: Reply<Vec<String>>,
    },
    Receive {
        mime: String,
        selection: Selection,
        reply: Reply<Result<UnixStream>>,
    },
    Watch {
        by: u64,
        watcher: Watcher,
    },
}

impl From<Request> for Command {
    fn from(request: Request) -> Command {
        Command::Request(request)
    }
}

pub(crate) type Worker = rikka_worker::Worker<CommandSender>;

pub(crate) fn start(host: Host, link: Link) -> Result<Worker> {
    let (sender, commands) = rikka_worker::command_channel(|| Command::Stop).map_err(Error::Io)?;
    Worker::spawn("rikka-clipboard", sender, move || run(host, commands, link)).map_err(startup)
}

fn run(host: Host, commands: CommandReceiver, link: Link) -> Result<impl FnOnce()> {
    let mut event_loop = host.display().event_loop::<State>();
    listen(&mut event_loop);
    let mut state = State::new();
    bind(&host, &event_loop.route(), link, &mut state)?;
    Ok(serve(host, event_loop, state, commands))
}

fn listen(event_loop: &mut EventLoop<State>) {
    event_loop.on::<ExtDataControlDeviceV1>(on_ext_data_control_device_v1);
    event_loop.on::<ExtDataControlOfferV1>(on_ext_data_control_offer_v1);
    event_loop.on::<ExtDataControlSourceV1>(on_ext_data_control_source_v1);
    event_loop.on::<WlDataDevice>(on_wl_data_device);
    event_loop.on::<WlDataOffer>(on_wl_data_offer);
    event_loop.on::<WlDataSource>(on_wl_data_source);
    event_loop.on::<ZwpPrimarySelectionDeviceV1>(on_zwp_primary_selection_device_v1);
    event_loop.on::<ZwpPrimarySelectionOfferV1>(on_zwp_primary_selection_offer_v1);
    event_loop.on::<ZwpPrimarySelectionSourceV1>(on_zwp_primary_selection_source_v1);
}

fn bind(host: &Host, route: &Route, link: Link, state: &mut State) -> Result<()> {
    let display = host.display();
    let seat: WlSeat = display.bind(1..=9).map_err(error::display)?;
    state.control = Control::bind(display, route, &seat);
    if state.control.is_some() {
        return Ok(());
    }
    if link == Link::Owned {
        return Err(Error::Unavailable("ext_data_control_manager_v1"));
    }
    let manager = display
        .bind_on::<WlDataDeviceManager>(route, 1..=3)
        .map_err(|_| Error::Unavailable("wl_data_device_manager"))?;
    state.primary_manager = display
        .bind_on::<ZwpPrimarySelectionDeviceManagerV1>(route, 1..=1)
        .ok();
    state.serials = Some(host.serials().map_err(error::display)?);
    state.device = Some(manager.get_data_device(&seat));
    state.primary_device = state
        .primary_manager
        .as_ref()
        .map(|manager| manager.get_device(&seat));
    state.manager = Some(manager);
    Ok(())
}

fn serve(
    host: Host,
    mut event_loop: EventLoop<State>,
    mut state: State,
    commands: CommandReceiver,
) -> impl FnOnce() {
    move || {
        let served = event_loop.run(&mut state, &commands, |state, command| match command {
            Command::Stop => ControlFlow::Break(()),
            Command::Request(request) => {
                handle_request(state, request);
                ControlFlow::Continue(())
            }
        });
        if let Err(error) = served {
            tracing::warn!(%error, "clipboard event loop stopped");
        }
        drop(host);
    }
}

fn handle_request(state: &mut State, request: Request) {
    match request {
        Request::Publish {
            content,
            selection,
            by,
        } => publish(state, content, selection, by),
        Request::Withdraw { selection, by } => withdraw(state, selection, by),
        Request::MimeTypes { selection, reply } => {
            let types = match (&state.control, selection) {
                (Some(control), _) => control.mime_types(selection),
                (None, Selection::Primary) => state
                    .primary_published
                    .mime_types_offered()
                    .unwrap_or_else(|| state.incoming.primary_mime_types.clone()),
                (None, Selection::Clipboard) => state
                    .published
                    .mime_types_offered()
                    .unwrap_or_else(|| state.incoming.mime_types.clone()),
            };
            send_reply(&reply, types);
        }
        Request::Receive {
            mime,
            selection,
            reply,
        } => send_reply(&reply, receive(state, &mime, selection)),
        Request::Watch { by, watcher } => state.watchers.push((by, watcher)),
    }
}

fn send_reply<T>(reply: &Reply<T>, value: T) {
    if reply.send(value).is_err() {
        tracing::debug!("clipboard requester stopped waiting");
    }
}

fn withdraw(state: &mut State, selection: Selection, by: u64) {
    let owner = match (&state.control, selection) {
        (Some(control), _) => control.owner(selection),
        (None, Selection::Primary) => state.primary_published.owner(),
        (None, Selection::Clipboard) => state.published.owner(),
    };
    if owner == Some(by) {
        publish(state, Content::default(), selection, by);
    }
}

fn publish(state: &mut State, content: Content, selection: Selection, by: u64) {
    if let Some(control) = state.control.as_mut() {
        return control.publish(content, selection, by);
    }

    let serial = state.serial();
    match selection {
        Selection::Primary => {
            let Some((manager, device)) = state
                .primary_manager
                .clone()
                .zip(state.primary_device.clone())
            else {
                return;
            };
            if let Some(old) = state.primary_published.source.take() {
                old.destroy();
            }
            let source = (!content.is_empty()).then(|| {
                let source = manager.create_source();
                for mime in content.mime_types() {
                    source.offer(&mime);
                }
                source
            });
            device.set_selection(source.as_ref(), serial);
            state.primary_published = Published {
                content,
                source,
                by,
            };
        }
        Selection::Clipboard => {
            let Some((manager, device)) = state.manager.clone().zip(state.device.clone()) else {
                return;
            };
            if let Some(old) = state.published.source.take() {
                old.destroy();
            }
            let source = (!content.is_empty()).then(|| {
                let source = manager.create_data_source();
                for mime in content.mime_types() {
                    source.offer(&mime);
                }
                source
            });
            device.set_selection(source.as_ref(), serial);
            state.published = Published {
                content,
                source,
                by,
            };
        }
    }
}

fn receive(state: &State, mime: &str, selection: Selection) -> Result<UnixStream> {
    if let Some(control) = &state.control {
        return control.receive(mime, selection);
    }

    match selection {
        Selection::Primary => {
            if let Some(stream) = state.primary_published.open(mime) {
                return stream;
            }
            ensure_offered(&state.incoming.primary_mime_types, mime)?;
            let offer = state.incoming.primary_offer.as_ref().ok_or(Error::Empty)?;
            transfer(|fd| offer.receive(mime, fd))
        }
        Selection::Clipboard => {
            if let Some(stream) = state.published.open(mime) {
                return stream;
            }
            ensure_offered(&state.incoming.mime_types, mime)?;
            let offer = state.incoming.offer.as_ref().ok_or(Error::Empty)?;
            transfer(|fd| offer.receive(mime, fd))
        }
    }
}
