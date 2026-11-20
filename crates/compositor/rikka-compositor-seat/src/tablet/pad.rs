use std::time::Duration;

use rikka_compositor_protocols::zwp_tablet_pad_ring_v2::Source as RingSource;
use rikka_compositor_protocols::zwp_tablet_pad_strip_v2::Source as StripSource;
use rikka_compositor_protocols::zwp_tablet_pad_v2::ButtonState;
use rikka_compositor_surface::SurfaceId;
use rikka_compositor_types::{PadInput, PadSource, Press};

use crate::access::Access;
use crate::ids::{PadId, SeatId};
use crate::input::millis;
use crate::surface;
use crate::tablet::PadObjects;

pub(crate) fn pad_focus<A: Access>(
    state: &mut A,
    seat: SeatId,
    pad: PadId,
    focus: Option<SurfaceId>,
) {
    let Some(old) = state
        .seats()
        .get(seat)
        .and_then(|entry| entry.tablets.pads.get(pad))
        .map(|pad_record| pad_record.focus)
    else {
        return;
    };
    if old == focus {
        return;
    }
    let serial = state.display().next_serial();
    if let Some(wl) = old.and_then(|old| surface::resource(state, old))
        && let Some(record) = state
            .seats()
            .seat_mut(seat)
            .and_then(|entry| entry.tablets.pads.get(pad))
    {
        for objects in objects_of(&record.objects, wl.client()) {
            objects.pad.leave(serial, &wl);
        }
    }
    let target = focus.and_then(|focus| surface::resource(state, focus).map(|wl| (focus, wl)));
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let tablets = &mut entry.tablets;
    let Some(record) = tablets.pads.get(pad) else {
        return;
    };
    let tablet = record.tablet;
    if let Some((_, wl)) = &target
        && let Some(tablet) = tablets.tablet_of(tablet, wl.client())
    {
        for objects in objects_of(&record.objects, wl.client()) {
            objects.pad.enter(serial, tablet, wl);
            for (index, group) in objects.groups.iter().enumerate() {
                let mode = record.modes.get(index).copied().unwrap_or(0);
                group.group.mode_switch(0, serial, mode);
            }
        }
    }
    if let Some(record) = tablets.pads.get_mut(pad) {
        record.focus = target.map(|(focus, _)| focus);
    }
}

pub(crate) fn pad_input<A: Access>(
    state: &mut A,
    seat: SeatId,
    pad: PadId,
    input: PadInput,
    time: Duration,
) {
    let serial = state.display().next_serial();
    let Some(focus) = state
        .seats()
        .get(seat)
        .and_then(|entry| entry.tablets.pads.get(pad))
        .and_then(|pad_record| pad_record.focus)
    else {
        if let PadInput::Mode { group, mode } = input {
            remember_mode(state, seat, pad, group, mode);
        }
        return;
    };
    let Some(client) = surface::resource(state, focus).map(|wl| wl.client()) else {
        return;
    };
    if let PadInput::Mode { group, mode } = input {
        remember_mode(state, seat, pad, group, mode);
    }
    let Some(record) = state
        .seats()
        .get(seat)
        .and_then(|entry| entry.tablets.pads.get(pad))
    else {
        return;
    };
    let time = millis(time);
    for objects in objects_of(&record.objects, client) {
        match input {
            PadInput::Button { button, press } => objects.pad.button(
                time,
                button,
                match press {
                    Press::Pressed => ButtonState::Pressed,
                    Press::Released => ButtonState::Released,
                },
            ),
            PadInput::Ring {
                ring,
                degrees,
                source,
            } => {
                for (_, resource) in objects
                    .groups
                    .iter()
                    .flat_map(|group| &group.rings)
                    .filter(|(index, _)| *index == ring)
                {
                    if source == PadSource::Finger {
                        resource.source(RingSource::Finger);
                    }
                    match degrees {
                        Some(degrees) => resource.angle(degrees),
                        None => resource.stop(),
                    }
                    resource.frame(time);
                }
            }
            PadInput::Strip {
                strip,
                position,
                source,
            } => {
                for (_, resource) in objects
                    .groups
                    .iter()
                    .flat_map(|group| &group.strips)
                    .filter(|(index, _)| *index == strip)
                {
                    if source == PadSource::Finger {
                        resource.source(StripSource::Finger);
                    }
                    match position {
                        Some(position) => {
                            resource.position((position.clamp(0.0, 1.0) * 65535.0) as u32)
                        }
                        None => resource.stop(),
                    }
                    resource.frame(time);
                }
            }
            PadInput::Dial { dial, v120 } => {
                for (_, resource) in objects
                    .groups
                    .iter()
                    .flat_map(|group| &group.dials)
                    .filter(|(index, _)| *index == dial)
                {
                    resource.delta(v120);
                    resource.frame(time);
                }
            }
            PadInput::Mode { group, mode } => {
                if let Some(found) = objects.groups.get(group as usize) {
                    found.group.mode_switch(time, serial, mode);
                }
            }
        }
    }
}

pub(crate) fn pads_surface_gone<A: Access>(state: &mut A, seat: SeatId, surface: SurfaceId) {
    if let Some(entry) = state.seats().seat_mut(seat) {
        for (_, pad) in entry.tablets.pads.iter_mut() {
            if pad.focus == Some(surface) {
                pad.focus = None;
            }
        }
    }
}

fn remember_mode<A: Access>(state: &mut A, seat: SeatId, pad: PadId, group: u32, mode: u32) {
    if let Some(slot) = state
        .seats()
        .seat_mut(seat)
        .and_then(|entry| entry.tablets.pads.get_mut(pad))
        .and_then(|pad_record| pad_record.modes.get_mut(group as usize))
    {
        *slot = mode;
    }
}

fn objects_of(
    objects: &[PadObjects],
    client: rikka_wayland_wire::ClientId,
) -> impl Iterator<Item = &PadObjects> {
    objects
        .iter()
        .filter(move |objects| objects.pad.client() == client && objects.pad.alive())
}
