use std::os::fd::AsFd;

use rikka_compositor_reactor::{Interest, Timestamp, WeakReactor};

use crate::commit::{acquired, drain, unblock};
use crate::host::SurfaceHost;

pub(crate) fn service<S: SurfaceHost>(state: &mut S, reactor: &WeakReactor<S>) {
    let Some(reactor) = reactor.upgrade() else {
        return;
    };
    let work = state.surfaces().blockers.waits.take_work();
    for (token, fd) in work.retired {
        reactor.remove(token);
        drop(fd);
    }
    for (blocker, fd) in work.fresh {
        let watched = reactor.watch(fd.as_fd(), Interest::READABLE, move |state: &mut S, _| {
            unblock(state, blocker);
        });
        match watched {
            Ok(token) => state.surfaces().blockers.waits.watching(blocker, token, fd),
            Err(error) => {
                tracing::debug!("failed to watch fence, treating it as signalled: {error}");
                unblock(state, blocker);
            }
        }
    }
    for surface in work.unwatched {
        let Some(fd) = state.surfaces().blockers.waits.acquirer_fd(surface) else {
            continue;
        };
        let watched = reactor.watch(fd, Interest::READABLE, move |state: &mut S, _| {
            acquired(state, surface, false);
        });
        match watched {
            Ok(token) => state
                .surfaces()
                .blockers
                .waits
                .acquirer_watched(surface, token),
            Err(error) => {
                tracing::debug!("failed to watch acquire point, treating it as signalled: {error}");
                acquired(state, surface, true);
                state.surfaces().blockers.waits.forget(surface);
            }
        }
    }
    let surfaces = state.surfaces();
    let deadline = surfaces.queue.next_deadline();
    let waits = &mut surfaces.blockers.waits;
    if waits.alarm.map(|(at, _)| at) == deadline {
        return;
    }
    if let Some((_, token)) = waits.alarm.take() {
        reactor.remove(token);
    }
    if let Some(at) = deadline {
        let token = reactor.timer(
            Timestamp::from_monotonic(at).to_instant(),
            |state: &mut S| {
                let surfaces = state.surfaces();
                surfaces.blockers.waits.alarm = None;
                surfaces.queue.reconsider();
                drain(state);
            },
        );
        waits.alarm = Some((at, token));
    }
}
