use rikka_compositor_reactor::Interest;
use rikka_wayland_wire::{Disconnect, Flush, Object, WireError, is_client_id};

use crate::client::Client;
use crate::display::DisplayInner;

pub(crate) fn maintain<S: 'static>(display: &DisplayInner<S>, state: &mut S) {
    let mut dirty = display.dirty.take();
    loop {
        tidy(display, state, None);
        display.core.take_dirty(&mut dirty);
        if dirty.is_empty() {
            break;
        }
        for id in dirty.drain(..) {
            let Some(client) = display.core.client(id) else {
                continue;
            };
            client.peer().clean();
            settle(display, state, &client);
            if client.alive() {
                flush(display, &client);
            }
        }
    }
    display.dirty.set(dirty);
}

pub(crate) fn settle<S: 'static>(display: &DisplayInner<S>, state: &mut S, client: &Client) {
    tidy(display, state, Some(client));
    if !client.alive() {
        teardown(display, state, client);
    }
}

pub(crate) fn tidy<S: 'static>(display: &DisplayInner<S>, state: &mut S, client: Option<&Client>) {
    let deferred = client.is_some_and(|client| client.peer().has_deferred_destroys());
    if !deferred && !display.core.has_binds() {
        return;
    }
    let mut deferred_destroys = display.deferred_destroys.take();
    loop {
        let mut progressed = false;
        if let Some(client) = client {
            client.peer().take_deferred_destroys(&mut deferred_destroys);
            progressed = !deferred_destroys.is_empty();
            for object in deferred_destroys.drain(..) {
                retire(display, state, client, &object);
            }
        }
        while let Some(object) = display.core.next_bind() {
            progressed = true;
            if !object.alive() {
                continue;
            }
            if let Some(dispatcher) = display.dispatcher(object.interface()) {
                dispatcher.bound(state, object);
            }
        }
        if !progressed {
            break;
        }
    }
    display.deferred_destroys.set(deferred_destroys);
}

pub(crate) fn retire<S: 'static>(
    display: &DisplayInner<S>,
    state: &mut S,
    client: &Client,
    object: &Object,
) {
    if !client.peer().retire(object) {
        return;
    }
    if is_client_id(object.id()) {
        client.display().delete_id(object.id());
    }
    if let Some(dispatcher) = display.dispatcher(object.interface()) {
        dispatcher.destroyed(state, object.clone());
    }
    client.peer().free(object);
}

pub(crate) fn flush<S: 'static>(display: &DisplayInner<S>, client: &Client) {
    let flushed = client.peer().connection().flush();
    let interest = match flushed {
        Ok(Flush::Done) if client.writable().replace(false) => Interest::READABLE,
        Ok(Flush::Pending) if !client.writable().replace(true) => Interest::BOTH,
        Ok(_) => return,
        Err(WireError::Io(error)) => {
            client.peer().close(Disconnect::Io(error.kind()));
            return;
        }
        Err(error) => {
            tracing::warn!(
                client = client.id().raw(),
                "failed to flush client: {error}"
            );
            return;
        }
    };
    if let Some(token) = client.watch().get()
        && let Err(error) = display.reactor.set_interest(token, interest)
    {
        tracing::warn!(
            client = client.id().raw(),
            "failed to update socket interest: {error}"
        );
    }
}

pub(crate) fn teardown<S: 'static>(display: &DisplayInner<S>, state: &mut S, client: &Client) {
    let reason = client.disconnect_reason().unwrap_or(Disconnect::Requested);
    if !display.core.dismiss(client.id()) {
        return;
    }
    if let Some(token) = client.watch().take() {
        display.reactor.remove(token);
    }
    {
        let connection = client.peer().connection();
        let _ = connection.flush();
        connection.close();
    }
    for object in client.peer().retire_all() {
        if let Some(dispatcher) = display.dispatcher(object.interface()) {
            dispatcher.destroyed(state, object);
        }
    }
    client.peer().free_all();
    tracing::debug!(client = client.id().raw(), "client disconnected: {reason}");
    display.disconnected(state, client, &reason);
}
