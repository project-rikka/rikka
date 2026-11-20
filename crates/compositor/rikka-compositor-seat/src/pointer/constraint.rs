use std::rc::Rc;

use rikka_compositor_geometry::{Logical, Point, Rect, Region, Size};
use rikka_compositor_protocols::zwp_confined_pointer_v1::{self as confined, ZwpConfinedPointerV1};
use rikka_compositor_protocols::zwp_locked_pointer_v1::{self as locked, ZwpLockedPointerV1};
use rikka_compositor_protocols::zwp_pointer_constraints_v1::{
    self as manager, Lifetime, ZwpPointerConstraintsV1,
};
use rikka_compositor_surface::{Committed, SurfaceId};

use crate::access::Access;
use crate::event::SeatEvent;
use crate::ids::SeatId;
use crate::surface;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConstraintKind {
    Lock,
    Confine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConstraintState {
    Locked,
    Confined,
    Released,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Handle {
    Lock(ZwpLockedPointerV1),
    Confine(ZwpConfinedPointerV1),
}

#[derive(Debug)]
pub(crate) struct Constraint {
    pub(crate) kind: ConstraintKind,
    pub(crate) handle: Handle,
    pub(crate) surface: SurfaceId,
    pub(crate) region: Option<Rc<Region<Logical>>>,
    pub(crate) pending_region: Option<Option<Rc<Region<Logical>>>>,
    pub(crate) clip: Vec<Rect<i32, Logical>>,
    pub(crate) clipped_to: Option<Size<i32, Logical>>,
    pub(crate) hint: Option<Point<f64, Logical>>,
    pub(crate) pending_hint: Option<Point<f64, Logical>>,
    pub(crate) oneshot: bool,
    pub(crate) active: bool,
    pub(crate) defunct: bool,
}

impl Constraint {
    fn reclip(&mut self, size: Option<Size<i32, Logical>>) {
        self.clipped_to = size;
        self.clip.clear();
        let Some(size) = size else {
            return;
        };
        let bounds = Rect::from_size(size);
        match &self.region {
            Some(region) => self
                .clip
                .extend(region.iter().filter_map(|rect| rect.intersection(bounds))),
            None => self.clip.push(bounds),
        }
    }
}

pub(crate) fn constraints_manager<A: Access>(
    state: &mut A,
    manager: ZwpPointerConstraintsV1,
    request: manager::Request,
) {
    let (handle, kind, surface, pointer, region, lifetime) = match request {
        manager::Request::Destroy => return,
        manager::Request::LockPointer {
            id,
            surface,
            pointer,
            region,
            lifetime,
        } => (
            Handle::Lock(id),
            ConstraintKind::Lock,
            surface,
            pointer,
            region,
            lifetime,
        ),
        manager::Request::ConfinePointer {
            id,
            surface,
            pointer,
            region,
            lifetime,
        } => (
            Handle::Confine(id),
            ConstraintKind::Confine,
            surface,
            pointer,
            region,
            lifetime,
        ),
    };
    let Some(seat) = pointer.key::<SeatId>() else {
        return;
    };
    let Some(surface) = surface::id(state, &surface) else {
        return;
    };
    match &handle {
        Handle::Lock(lock) => lock.set_key(seat),
        Handle::Confine(confine) => confine.set_key(seat),
    }
    let region = region
        .as_ref()
        .map(|region| state.surfaces().region(region));
    let size = size_of(state, surface);
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    if entry
        .pointer
        .constraints
        .iter()
        .any(|constraint| constraint.surface == surface)
    {
        manager.post_error(manager::Error::AlreadyConstrained);
        return;
    }
    let mut constraint = Constraint {
        kind,
        handle,
        surface,
        region,
        pending_region: None,
        clip: Vec::new(),
        clipped_to: None,
        hint: None,
        pending_hint: None,
        oneshot: !matches!(lifetime, Ok(Lifetime::Persistent)),
        active: false,
        defunct: false,
    };
    constraint.reclip(size);
    entry.pointer.constraints.push(constraint);
    refresh(state, seat);
}

pub(crate) fn locked_request<A: Access>(
    state: &mut A,
    lock: ZwpLockedPointerV1,
    request: locked::Request,
) {
    let handle = Handle::Lock(lock);
    match request {
        locked::Request::Destroy => {}
        locked::Request::SetCursorPositionHint {
            surface_x,
            surface_y,
        } => with_constraint(state, &handle, |constraint| {
            constraint.pending_hint = Some(Point::new(surface_x, surface_y));
        }),
        locked::Request::SetRegion { region } => {
            let region = region
                .as_ref()
                .map(|region| state.surfaces().region(region));
            with_constraint(state, &handle, |constraint| {
                constraint.pending_region = Some(region);
            });
        }
    }
}

pub(crate) fn confined_request<A: Access>(
    state: &mut A,
    confine: ZwpConfinedPointerV1,
    request: confined::Request,
) {
    let handle = Handle::Confine(confine);
    match request {
        confined::Request::Destroy => {}
        confined::Request::SetRegion { region } => {
            let region = region
                .as_ref()
                .map(|region| state.surfaces().region(region));
            with_constraint(state, &handle, |constraint| {
                constraint.pending_region = Some(region);
            });
        }
    }
}

pub(crate) fn constraint_destroyed<A: Access>(state: &mut A, handle: Handle) {
    let seat = match &handle {
        Handle::Lock(lock) => lock.key::<SeatId>(),
        Handle::Confine(confine) => confine.key::<SeatId>(),
    };
    let Some(seat) = seat else {
        return;
    };
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let Some(index) = entry
        .pointer
        .constraints
        .iter()
        .position(|constraint| constraint.handle == handle)
    else {
        return;
    };
    let constraint = entry.pointer.constraints.remove(index);
    if constraint.active {
        announce_release(state, seat, &constraint);
    }
}

pub(crate) fn constraints_committed<A: Access>(state: &mut A, seat: SeatId, commit: &Committed) {
    let size = size_of(state, commit.surface);
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let mut touched = false;
    for constraint in entry
        .pointer
        .constraints
        .iter_mut()
        .filter(|constraint| constraint.surface == commit.surface)
    {
        let reshaped = match constraint.pending_region.take() {
            Some(region) => {
                constraint.region = region;
                true
            }
            None => false,
        };
        if reshaped || constraint.clipped_to != size {
            constraint.reclip(size);
        }
        touched |= reshaped;
        if let Some(hint) = constraint.pending_hint.take() {
            constraint.hint = Some(hint);
        }
    }
    if touched {
        refresh(state, seat);
    }
}

pub(crate) fn constraints_surface_gone<A: Access>(state: &mut A, seat: SeatId, surface: SurfaceId) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let constraints = &mut entry.pointer.constraints;
    let Some(index) = constraints
        .iter()
        .position(|constraint| constraint.surface == surface)
    else {
        return;
    };
    let gone = constraints.remove(index);
    if gone.active {
        announce_release(state, seat, &gone);
    }
}

pub(crate) fn deactivate_constraints_except<A: Access>(
    state: &mut A,
    seat: SeatId,
    target: Option<SurfaceId>,
) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let origin = entry.pointer.focus().map(|focus| focus.origin);
    let Some(constraint) = entry
        .pointer
        .constraints
        .iter_mut()
        .find(|constraint| constraint.active && Some(constraint.surface) != target)
    else {
        return;
    };
    constraint.active = false;
    constraint.defunct = constraint.oneshot;
    match &constraint.handle {
        Handle::Lock(lock) => lock.unlocked(),
        Handle::Confine(confine) => confine.unconfined(),
    }
    let surface = constraint.surface;
    let hint = (constraint.kind == ConstraintKind::Lock)
        .then_some(constraint.hint)
        .flatten()
        .zip(origin)
        .map(|(hint, origin)| origin + hint);
    state.report(SeatEvent::Constraint {
        seat,
        surface,
        state: ConstraintState::Released,
    });
    if let Some(location) = hint {
        state.report(SeatEvent::CursorHint { seat, location });
    }
}

pub(crate) fn refresh<A: Access>(state: &mut A, seat: SeatId) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let pointer = &mut entry.pointer;
    if pointer.constraints.is_empty() {
        return;
    }
    let Some(focus) = pointer.focus() else {
        return;
    };
    let local = focus.local(pointer.location);
    let inside = Point::<i32, Logical>::new(floor(local.x), floor(local.y));
    let Some(constraint) = pointer.constraints.iter_mut().find(|constraint| {
        constraint.surface == focus.surface && !constraint.active && !constraint.defunct
    }) else {
        return;
    };
    if constraint
        .region
        .as_ref()
        .is_some_and(|region| !region.contains(inside))
    {
        return;
    }
    constraint.active = true;
    let (surface, announced) = match &constraint.handle {
        Handle::Lock(lock) => {
            lock.locked();
            (constraint.surface, ConstraintState::Locked)
        }
        Handle::Confine(confine) => {
            confine.confined();
            (constraint.surface, ConstraintState::Confined)
        }
    };
    state.report(SeatEvent::Constraint {
        seat,
        surface,
        state: announced,
    });
}

pub(crate) fn constrain<A: Access>(
    state: &mut A,
    seat: SeatId,
    proposed: Point<f64, Logical>,
) -> Point<f64, Logical> {
    let Some(entry) = state.seats().get(seat) else {
        return proposed;
    };
    let pointer = &entry.pointer;
    let Some(focus) = pointer.focus() else {
        return proposed;
    };
    let Some(constraint) = pointer
        .constraints
        .iter()
        .find(|constraint| constraint.active && constraint.surface == focus.surface)
    else {
        return proposed;
    };
    if constraint.kind == ConstraintKind::Lock {
        return pointer.location;
    }
    let local = proposed - focus.origin;
    constraint
        .clip
        .iter()
        .map(|rect| clamp(local, *rect))
        .min_by(|a, b| distance(*a, local).total_cmp(&distance(*b, local)))
        .map_or(proposed, |point| point + focus.origin)
}

fn size_of(state: &mut impl Access, surface: SurfaceId) -> Option<Size<i32, Logical>> {
    state
        .surfaces()
        .get(surface)
        .and_then(|surface| surface.size())
}

fn floor(value: f64) -> i32 {
    let truncated = value as i32;
    if f64::from(truncated) > value {
        truncated - 1
    } else {
        truncated
    }
}

fn clamp(point: Point<f64, Logical>, rect: Rect<i32, Logical>) -> Point<f64, Logical> {
    let rect = rect.to_f64();
    let right = (rect.loc.x + rect.size.w - 1.0 / 256.0).max(rect.loc.x);
    let bottom = (rect.loc.y + rect.size.h - 1.0 / 256.0).max(rect.loc.y);
    Point::new(
        point.x.clamp(rect.loc.x, right),
        point.y.clamp(rect.loc.y, bottom),
    )
}

fn distance(a: Point<f64, Logical>, b: Point<f64, Logical>) -> f64 {
    (a.x - b.x).powi(2) + (a.y - b.y).powi(2)
}

fn announce_release<A: Access>(state: &mut A, seat: SeatId, constraint: &Constraint) {
    let origin = state
        .seats()
        .get(seat)
        .and_then(|entry| entry.pointer.focus())
        .filter(|focus| focus.surface == constraint.surface)
        .map(|focus| focus.origin);
    state.report(SeatEvent::Constraint {
        seat,
        surface: constraint.surface,
        state: ConstraintState::Released,
    });
    if constraint.kind == ConstraintKind::Lock
        && let Some((hint, origin)) = constraint.hint.zip(origin)
    {
        state.report(SeatEvent::CursorHint {
            seat,
            location: origin + hint,
        });
    }
}

fn with_constraint<A: Access>(state: &mut A, handle: &Handle, visit: impl FnOnce(&mut Constraint)) {
    let seat = match handle {
        Handle::Lock(lock) => lock.key::<SeatId>(),
        Handle::Confine(confine) => confine.key::<SeatId>(),
    };
    let Some(entry) = seat.and_then(|seat| state.seats().seat_mut(seat)) else {
        return;
    };
    if let Some(constraint) = entry
        .pointer
        .constraints
        .iter_mut()
        .find(|constraint| constraint.handle == *handle)
    {
        visit(constraint);
    }
}
