use std::time::Duration;

use rikka_compositor_protocols::zwp_tablet_tool_v2::{ButtonState, ZwpTabletToolV2};
use rikka_compositor_surface::SurfaceId;
use rikka_compositor_types::{Press, ToolAxes};
use smallvec::SmallVec;

use crate::access::Access;
use crate::focus::{Focus, Focused};
use crate::ids::{SeatId, TabletId, ToolId};
use crate::input::millis;
use crate::pointer::{CursorImage, CursorOwner, show};
use crate::surface;
use crate::tablet::Tool;
use crate::tablet::info::ToolMotion;

pub(crate) fn tool_motion<A: Access>(
    state: &mut A,
    seat: SeatId,
    tool: ToolId,
    tablet: TabletId,
    motion: &ToolMotion,
) {
    let Some((focus, down)) = state.seats().get(seat).and_then(|entry| {
        let record = entry.tablets.tools.get(tool)?;
        Some((
            record.focused.as_ref().map(|focused| focused.focus),
            record.down,
        ))
    }) else {
        return;
    };
    let target = if down { focus } else { motion.under };
    if focus.map(|focus| focus.surface) != target.map(|focus| focus.surface) {
        if focus.is_some() {
            leave(state, seat, tool, motion.time);
        }
        if let Some(target) = target {
            enter(state, seat, tool, tablet, target);
        }
    }
    let Some(record) = tool_mut(state, seat, tool) else {
        return;
    };
    let Some(focused) = record.focused.as_mut() else {
        return;
    };
    if let Some(target) = target {
        focused.focus.origin = target.origin;
    }
    let local = focused.focus.local(motion.location);
    for resource in record.resources.for_client(focused.client()) {
        resource.motion(local.x, local.y);
        send_axes(resource, &motion.axes);
        resource.frame(millis(motion.time));
    }
}

pub(crate) fn tool_out<A: Access>(state: &mut A, seat: SeatId, tool: ToolId, time: Duration) {
    if tool_mut(state, seat, tool).is_some_and(|tool_record| tool_record.focused.is_some()) {
        leave(state, seat, tool, time);
    }
}

pub(crate) fn tool_tip<A: Access>(
    state: &mut A,
    seat: SeatId,
    tool: ToolId,
    press: Press,
    time: Duration,
) {
    let Some(record) = tool_mut(state, seat, tool) else {
        return;
    };
    record.down = press == Press::Pressed;
    if record.focused.is_none() {
        return;
    }
    let serial = state.display().next_serial();
    let Some(record) = tool_mut(state, seat, tool) else {
        return;
    };
    for resource in focused_resources(record) {
        match press {
            Press::Pressed => resource.down(serial),
            Press::Released => resource.up(),
        }
        resource.frame(millis(time));
    }
}

pub(crate) fn tool_button<A: Access>(
    state: &mut A,
    seat: SeatId,
    tool: ToolId,
    button: u32,
    press: Press,
    time: Duration,
) {
    if tool_mut(state, seat, tool).is_none_or(|tool_record| tool_record.focused.is_none()) {
        return;
    }
    let serial = state.display().next_serial();
    let pressed = match press {
        Press::Pressed => ButtonState::Pressed,
        Press::Released => ButtonState::Released,
    };
    let Some(record) = tool_mut(state, seat, tool) else {
        return;
    };
    for resource in focused_resources(record) {
        resource.button(serial, button, pressed);
        resource.frame(millis(time));
    }
}

pub(crate) fn tools_surface_gone<A: Access>(state: &mut A, seat: SeatId, surface: SurfaceId) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let gone: SmallVec<[ToolId; 2]> = entry
        .tablets
        .tools
        .iter_mut()
        .filter(|(_, tool)| {
            tool.focused
                .as_ref()
                .is_some_and(|focused| focused.focus.surface == surface)
        })
        .map(|(id, tool)| {
            tool.focused = None;
            tool.down = false;
            id
        })
        .collect();
    for tool in gone {
        show(
            state,
            seat,
            CursorOwner::Tool(tool),
            CursorImage::default(),
            None,
        );
    }
}

fn enter<A: Access>(state: &mut A, seat: SeatId, tool: ToolId, tablet: TabletId, target: Focus) {
    let Some(wl) = surface::resource(state, target.surface) else {
        return;
    };
    let serial = state.display().next_serial();
    let client = wl.client();
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let Some(tablet_resource) = entry.tablets.tablet_of(tablet, client).cloned() else {
        return;
    };
    let Some(record) = entry.tablets.tools.get_mut(tool) else {
        return;
    };
    record.tablet = Some(tablet);
    record.proximity_serial = serial;
    for resource in record.resources.for_client(client) {
        resource.proximity_in(serial, &tablet_resource, &wl);
    }
    record.focused = Some(Focused {
        focus: target,
        surface: wl,
    });
}

fn leave<A: Access>(state: &mut A, seat: SeatId, tool: ToolId, time: Duration) {
    let Some(record) = tool_mut(state, seat, tool) else {
        return;
    };
    let was_down = std::mem::replace(&mut record.down, false);
    for resource in focused_resources(record) {
        if was_down {
            resource.up();
        }
        resource.proximity_out();
        resource.frame(millis(time));
    }
    record.focused = None;
    show(
        state,
        seat,
        CursorOwner::Tool(tool),
        CursorImage::default(),
        None,
    );
}

fn tool_mut<A: Access>(state: &mut A, seat: SeatId, tool: ToolId) -> Option<&mut Tool> {
    state.seats().seat_mut(seat)?.tablets.tools.get_mut(tool)
}

fn focused_resources(record: &Tool) -> &[ZwpTabletToolV2] {
    record
        .focused
        .as_ref()
        .map_or(&[], |focused| record.resources.for_client(focused.client()))
}

fn send_axes(resource: &ZwpTabletToolV2, axes: &ToolAxes) {
    if let Some(pressure) = axes.pressure {
        resource.pressure((pressure.clamp(0.0, 1.0) * 65535.0) as u32);
    }
    if let Some(distance) = axes.distance {
        resource.distance((distance.clamp(0.0, 1.0) * 65535.0) as u32);
    }
    if let Some((x, y)) = axes.tilt {
        resource.tilt(x, y);
    }
    if let Some(degrees) = axes.rotation {
        resource.rotation(degrees);
    }
    if let Some(slider) = axes.slider {
        resource.slider((slider.clamp(-1.0, 1.0) * 65535.0) as i32);
    }
    if let Some((degrees, clicks)) = axes.wheel {
        resource.wheel(degrees, clicks);
    }
}
