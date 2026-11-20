use std::io;
use std::os::fd::AsFd;
use std::os::unix::net::UnixListener;
use std::rc::Rc;

use rikka_compositor_reactor::{Interest, Reactor, Token};

use super::state::State;
use crate::event::Event;

pub(crate) fn listen(reactor: &Reactor<State>, listener: UnixListener) -> io::Result<Token> {
    let listener = Rc::new(listener);
    let shared = Rc::clone(&listener);
    reactor.watch(listener.as_fd(), Interest::READABLE, move |state, _| {
        accept_waiting(state, &shared);
    })
}

fn accept_waiting(state: &mut State, listener: &UnixListener) {
    loop {
        match listener.accept() {
            Ok((stream, _)) => admit(state, stream),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => {
                tracing::warn!("failed to accept nested client: {error}");
                return;
            }
        }
    }
}

fn admit(state: &mut State, stream: std::os::unix::net::UnixStream) {
    let peer = match rustix::net::sockopt::socket_peercred(&stream) {
        Ok(peer) => peer,
        Err(error) => {
            tracing::warn!("failed to read nested client credentials: {error}");
            return;
        }
    };
    let ours = rustix::process::getuid();
    if peer.uid != ours {
        state.event_sender.send(Event::ClientRefused {
            uid: peer.uid.as_raw(),
        });
        return;
    }
    match state.display.insert_client(stream, State::client_options()) {
        Ok(_) => state.event_sender.send(Event::ClientConnected {
            pid: peer.pid.as_raw_nonzero().get(),
        }),
        Err(error) => tracing::warn!("failed to add nested client: {error}"),
    }
}
