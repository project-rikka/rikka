use rikka_compositor_protocols::wl_display;
use rikka_compositor_reactor::Ready;
use rikka_wayland_wire::{
    Disconnect, Fill, Header, Interface, MessageSpec, Object, Resource, Signature, WireError,
};

use crate::client::Client;
use crate::display::DisplayInner;
use crate::upkeep;

pub(crate) fn ready<S: 'static>(
    display: &DisplayInner<S>,
    state: &mut S,
    client: &Client,
    ready: Ready,
) {
    if ready.writable {
        upkeep::flush(display, client);
    }
    if ready.readable {
        let filled = client.peer().connection().fill();
        match filled {
            Ok(Fill::Read(_) | Fill::Full) => requests(display, state, client),
            Ok(Fill::WouldBlock) => {}
            Ok(Fill::Closed) => client.peer().close(Disconnect::Hangup),
            Err(WireError::Io(error)) => client.peer().close(Disconnect::Io(error.kind())),
            Err(error) => malformed(client, &error),
        }
    } else if ready.hangup || ready.error {
        client.peer().close(Disconnect::Hangup);
    }
    upkeep::settle(display, state, client);
}

fn requests<S: 'static>(display: &DisplayInner<S>, state: &mut S, client: &Client) {
    let peer = client.peer();
    while client.alive() {
        upkeep::tidy(display, state, Some(client));
        let header = match peer.connection().peek() {
            Ok(Some(header)) => header,
            Ok(None) => break,
            Err(error) => {
                malformed(client, &error);
                break;
            }
        };
        let Some(object) = peer.object(header.object) else {
            unknown_object(client, header.object);
            break;
        };
        let interface = object.interface();
        let Some(spec) = interface.requests.get(usize::from(header.opcode)) else {
            unknown_opcode(client, &object, header.opcode);
            break;
        };
        if object.version() < spec.since {
            too_new(client, &object, spec);
            break;
        }
        let Some(dispatcher) = display.dispatcher(interface) else {
            unserved(client, interface);
            break;
        };
        dispatcher.request(state, client, object.clone(), header, spec.signature);
        if spec.destructor {
            upkeep::retire(display, state, client, &object);
        }
        display.reactor.settle(state);
    }
}

pub(crate) fn request<S, R: Resource>(
    state: &mut S,
    client: &Client,
    object: Object,
    header: Header,
    signature: Signature,
    handle: impl for<'m> FnOnce(&mut S, R, R::Request<'m>),
) {
    let peer = client.peer();
    let mut message = peer.connection().message(header);
    let decoded = {
        let mut objects = peer.objects();
        let mut args = message.args(signature);
        R::decode(&object, header.opcode, &mut args, &mut objects)
            .and_then(|request| args.finish().map(|()| request))
    };
    match decoded {
        Ok(request) => {
            if let Some(resource) = R::from_object(object) {
                handle(state, resource, request);
            }
        }
        Err(error) => reject(client, &object, header.opcode, &error),
    }
}

#[cold]
#[inline(never)]
fn malformed(client: &Client, error: &WireError) {
    client.post_display_error(wl_display::Error::InvalidMethod, &error.to_string());
}

#[cold]
#[inline(never)]
fn unknown_object(client: &Client, id: u32) {
    client.post_display_error(
        wl_display::Error::InvalidObject,
        &format!("invalid object {id}"),
    );
}

#[cold]
#[inline(never)]
fn unknown_opcode(client: &Client, object: &Object, opcode: u16) {
    client.post_display_error(
        wl_display::Error::InvalidMethod,
        &format!(
            "invalid method {opcode}, object {}#{}",
            object.interface().name,
            object.id()
        ),
    );
}

#[cold]
#[inline(never)]
fn too_new(client: &Client, object: &Object, spec: &MessageSpec) {
    client.post_display_error(
        wl_display::Error::InvalidMethod,
        &format!(
            "invalid version for {}#{}.{} ({}, need at least {})",
            object.interface().name,
            object.id(),
            spec.name,
            object.version(),
            spec.since
        ),
    );
}

#[cold]
#[inline(never)]
fn unserved(client: &Client, interface: &Interface) {
    client.kill(&format!(
        "{} is not served by this compositor",
        interface.name
    ));
}

#[cold]
#[inline(never)]
fn reject(client: &Client, object: &Object, opcode: u16, error: &WireError) {
    let interface = object.interface();
    let name = interface
        .requests
        .get(usize::from(opcode))
        .map_or("unknown", |spec| spec.name);
    match error {
        WireError::InvalidNewId(id) => client.post_display_error(
            wl_display::Error::InvalidObject,
            &format!("invalid new id {id}"),
        ),
        error => client.post_display_error(
            wl_display::Error::InvalidMethod,
            &format!(
                "invalid arguments for {}#{}.{name}: {error}",
                interface.name,
                object.id()
            ),
        ),
    }
}
