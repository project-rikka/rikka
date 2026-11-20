use rikka_compositor_reactor::Timestamp;

use crate::cached;
use crate::committed::Committed;
use crate::host::SurfaceHost;
use crate::ids::{BlockerId, SurfaceId};

pub fn unblock<S: SurfaceHost>(state: &mut S, blocker: BlockerId) {
    let surfaces = state.surfaces();
    if surfaces.blockers.release(blocker) {
        surfaces.queue.unblocked(blocker);
        drain(state);
    }
}

pub(crate) fn acquired<S: SurfaceHost>(state: &mut S, surface: SurfaceId, all: bool) {
    let surfaces = state.surfaces();
    let reached = surfaces.blockers.waits.acquired(surface, all);
    for blocker in reached {
        if surfaces.blockers.release(blocker) {
            surfaces.queue.unblocked(blocker);
        }
    }
    drain(state);
}

pub(crate) fn commit<S: SurfaceHost>(state: &mut S, id: SurfaceId) {
    let Some(check) = state.surfaces().validate(id) else {
        return;
    };
    if let Some(hooks) = state.surfaces().hooks::<S>(id)
        && (hooks.pre_commit)(state, &check).is_err()
    {
        return;
    }
    state.committing(id);
    let surfaces = state.surfaces();
    let Some(mut update) = surfaces.take_pending(id) else {
        return;
    };
    if check.synced {
        update.constraints.wait_barrier = false;
        surfaces.cache(id, update);
        return;
    }
    surfaces.enqueue(id, update);
    drain(state);
}

pub(crate) fn flush_cached<S: SurfaceHost>(state: &mut S, id: SurfaceId) {
    state.surfaces().enqueue_cached(id);
    drain(state);
}

pub(crate) fn destroy<S: SurfaceHost>(state: &mut S, id: SurfaceId) {
    let active = state
        .surfaces()
        .surfaces
        .get_mut(id)
        .and_then(|surface| surface.role.as_mut())
        .and_then(|role| role.take_active::<S>());
    if let Some((hooks, role)) = active {
        (hooks.destroyed)(state, id, role);
    }
    state.destroyed(id);
    state.surfaces().remove(id);
    drain(state);
}

pub(crate) fn drain<S: SurfaceHost>(state: &mut S) {
    let surfaces = state.surfaces();
    if !surfaces.queue.begin_drain() {
        return;
    }
    let mut commits = std::mem::take(&mut surfaces.commits);
    loop {
        commits.clear();
        if !state.surfaces().apply_next(&mut commits) {
            break;
        }
        for commit in &commits {
            notify(state, commit);
        }
    }
    let surfaces = state.surfaces();
    surfaces.commits = commits;
    surfaces.queue.end_drain();
    let mut awaited = std::mem::take(&mut surfaces.awaited);
    surfaces.outputs_awaited(Timestamp::now().as_monotonic(), &mut awaited);
    for output in &awaited {
        state.wants_refresh(*output);
    }
    state.surfaces().awaited = awaited;
}

fn notify<S: SurfaceHost>(state: &mut S, commit: &Committed) {
    while let Some(hook) = state.surfaces().next_applied(commit.surface) {
        cached::run(state, commit.surface, hook);
    }
    if let Some(hooks) = state.surfaces().hooks::<S>(commit.surface) {
        (hooks.committed)(state, commit);
    }
    state.committed(commit);
}
