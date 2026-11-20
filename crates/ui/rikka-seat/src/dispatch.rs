use std::time::Instant;

use rikka_wayland::SerialKind;
use rikka_wayland_client::Queue;
use rikka_wayland_client::wl_keyboard::{self, KeyState, WlKeyboard};
use rikka_wayland_client::wl_pointer::{self, ButtonState, WlPointer};
use rikka_wayland_client::wl_seat::{self, WlSeat};
use rikka_wayland_client::wl_touch::{self, WlTouch};
use rikka_wayland_client::zwp_pointer_gesture_hold_v1::{self, ZwpPointerGestureHoldV1};
use rikka_wayland_client::zwp_pointer_gesture_pinch_v1::{self, ZwpPointerGesturePinchV1};
use rikka_wayland_client::zwp_pointer_gesture_swipe_v1::{self, ZwpPointerGestureSwipeV1};
use rikka_wayland_client::zwp_relative_pointer_v1::{self, ZwpRelativePointerV1};
use rikka_wayland_client::zwp_tablet_pad_dial_v2::{self, ZwpTabletPadDialV2};
use rikka_wayland_client::zwp_tablet_pad_group_v2::{self, ZwpTabletPadGroupV2};
use rikka_wayland_client::zwp_tablet_pad_ring_v2::{self, ZwpTabletPadRingV2};
use rikka_wayland_client::zwp_tablet_pad_strip_v2::{self, ZwpTabletPadStripV2};
use rikka_wayland_client::zwp_tablet_pad_v2::{self, ZwpTabletPadV2};
use rikka_wayland_client::zwp_tablet_seat_v2::{self, ZwpTabletSeatV2};
use rikka_wayland_client::zwp_tablet_tool_v2::{self, ZwpTabletToolV2};
use rikka_wayland_client::zwp_tablet_v2::{self, ZwpTabletV2};
use rikka_wayland_client::zwp_text_input_v3::{self, ZwpTextInputV3};

use crate::host::SeatHost;
use crate::pad::PadDevice;
use crate::pointer::Pointer;
use crate::seat::Seat;
use crate::seat_event::SeatEvent;
use crate::seats::{Seats, deliver};

pub(crate) fn listen<D: SeatHost>(queue: &mut Queue<D>) {
    queue.on::<WlSeat>(on_wl_seat::<D>);
    queue.on::<WlKeyboard>(on_wl_keyboard::<D>);
    queue.on::<WlPointer>(on_wl_pointer::<D>);
    queue.on::<WlTouch>(on_wl_touch::<D>);
    queue.on::<ZwpPointerGestureSwipeV1>(on_zwp_pointer_gesture_swipe_v1::<D>);
    queue.on::<ZwpPointerGesturePinchV1>(on_zwp_pointer_gesture_pinch_v1::<D>);
    queue.on::<ZwpPointerGestureHoldV1>(on_zwp_pointer_gesture_hold_v1::<D>);
    queue.on::<ZwpRelativePointerV1>(on_zwp_relative_pointer_v1::<D>);
    queue.on::<ZwpTextInputV3>(on_zwp_text_input_v3::<D>);
    queue.on::<ZwpTabletSeatV2>(on_zwp_tablet_seat_v2::<D>);
    queue.on::<ZwpTabletV2>(on_zwp_tablet_v2::<D>);
    queue.on::<ZwpTabletToolV2>(on_zwp_tablet_tool_v2::<D>);
    queue.on::<ZwpTabletPadV2>(on_zwp_tablet_pad_v2::<D>);
    queue.on::<ZwpTabletPadGroupV2>(on_zwp_tablet_pad_group_v2::<D>);
    queue.on::<ZwpTabletPadRingV2>(on_zwp_tablet_pad_ring_v2::<D>);
    queue.on::<ZwpTabletPadStripV2>(on_zwp_tablet_pad_strip_v2::<D>);
    queue.on::<ZwpTabletPadDialV2>(on_zwp_tablet_pad_dial_v2::<D>);
}

fn collect_events<D: SeatHost>(host: &mut D, handle: impl FnOnce(&mut Seats, &mut Vec<SeatEvent>)) {
    let seats = host.seats();
    let mut events = std::mem::take(&mut seats.events);
    seats.flush_tablets(&mut events);
    handle(seats, &mut events);
    deliver(host, events);
}

fn collect_tablet_events<D: SeatHost>(
    host: &mut D,
    handle: impl FnOnce(&mut Seats, &mut Vec<SeatEvent>),
) {
    let seats = host.seats();
    let mut events = std::mem::take(&mut seats.events);
    handle(seats, &mut events);
    deliver(host, events);
}

fn pointer_matching(seats: &mut [Seat], owns: impl Fn(&Pointer) -> bool) -> Option<&mut Pointer> {
    seats
        .iter_mut()
        .filter_map(|seat| seat.pointer.as_mut())
        .find(|pointer| owns(pointer))
}

fn on_wl_seat<D: SeatHost>(host: &mut D, seat: WlSeat, event: wl_seat::Event) {
    let wl_seat::Event::Capabilities { capabilities } = event else {
        return;
    };
    let Seats {
        seats, managers, ..
    } = host.seats();
    if let Some(found) = seats.iter_mut().find(|found| found.seat == seat) {
        found.set_capabilities(capabilities, managers);
    }
}

fn on_wl_keyboard<D: SeatHost>(host: &mut D, keyboard: WlKeyboard, event: wl_keyboard::Event) {
    let now = Instant::now();
    let stroke = keyboard_stroke(&event);
    collect_events(host, |seats, events| {
        let Some(seat) = seats.seats.iter_mut().find(|seat| {
            seat.keyboard
                .as_ref()
                .is_some_and(|owner| owner.keyboard == keyboard)
        }) else {
            return;
        };
        let wl_seat = seat.seat.clone();
        if let Some(owner) = seat.keyboard.as_mut() {
            owner.handle(event, &mut seat.modifiers, now, events);
        }
        seats.record(&wl_seat, stroke);
    });
}

fn on_wl_pointer<D: SeatHost>(host: &mut D, pointer: WlPointer, event: wl_pointer::Event) {
    let stroke = pointer_stroke(&event);
    collect_events(host, |seats, events| {
        let Some(seat) = seats.seats.iter_mut().find(|seat| {
            seat.pointer
                .as_ref()
                .is_some_and(|owner| owner.pointer == pointer)
        }) else {
            return;
        };
        let modifiers = seat.modifiers;
        let wl_seat = seat.seat.clone();
        let pressed = seat
            .pointer
            .as_mut()
            .and_then(|owner| owner.handle(event, modifiers, events));
        if let Some(serial) = pressed {
            seats.pressed(&wl_seat, serial);
        }
        seats.record(&wl_seat, stroke);
    });
}

fn on_wl_touch<D: SeatHost>(host: &mut D, touch: WlTouch, event: wl_touch::Event) {
    collect_events(host, |seats, events| {
        let Some(seat) = seats.seats.iter_mut().find(|seat| {
            seat.touch
                .as_ref()
                .is_some_and(|owner| owner.touch == touch)
        }) else {
            return;
        };
        let modifiers = seat.modifiers;
        let wl_seat = seat.seat.clone();
        let pressed = seat
            .touch
            .as_mut()
            .and_then(|owner| owner.handle(event, modifiers, events));
        if let Some(serial) = pressed {
            seats.pressed(&wl_seat, serial);
        }
    });
}

fn on_zwp_pointer_gesture_swipe_v1<D: SeatHost>(
    host: &mut D,
    swipe: ZwpPointerGestureSwipeV1,
    event: zwp_pointer_gesture_swipe_v1::Event,
) {
    collect_events(host, |seats, events| {
        let owns = |pointer: &Pointer| {
            pointer
                .gestures
                .as_ref()
                .is_some_and(|gestures| gestures.swipe == swipe)
        };
        if let Some(pointer) = pointer_matching(&mut seats.seats, owns) {
            let gesture = pointer
                .gestures
                .as_mut()
                .and_then(|g| g.handle_swipe(event));
            pointer.gesture(gesture, events);
        }
    });
}

fn on_zwp_pointer_gesture_pinch_v1<D: SeatHost>(
    host: &mut D,
    pinch: ZwpPointerGesturePinchV1,
    event: zwp_pointer_gesture_pinch_v1::Event,
) {
    collect_events(host, |seats, events| {
        let owns = |pointer: &Pointer| {
            pointer
                .gestures
                .as_ref()
                .is_some_and(|gestures| gestures.pinch == pinch)
        };
        if let Some(pointer) = pointer_matching(&mut seats.seats, owns) {
            let gesture = pointer
                .gestures
                .as_mut()
                .and_then(|g| g.handle_pinch(event));
            pointer.gesture(gesture, events);
        }
    });
}

fn on_zwp_pointer_gesture_hold_v1<D: SeatHost>(
    host: &mut D,
    hold: ZwpPointerGestureHoldV1,
    event: zwp_pointer_gesture_hold_v1::Event,
) {
    collect_events(host, |seats, events| {
        let owns = |pointer: &Pointer| {
            pointer
                .gestures
                .as_ref()
                .is_some_and(|gestures| gestures.hold.as_ref() == Some(&hold))
        };
        if let Some(pointer) = pointer_matching(&mut seats.seats, owns) {
            let gesture = pointer.gestures.as_mut().and_then(|g| g.handle_hold(event));
            pointer.gesture(gesture, events);
        }
    });
}

fn on_zwp_relative_pointer_v1<D: SeatHost>(
    host: &mut D,
    relative: ZwpRelativePointerV1,
    event: zwp_relative_pointer_v1::Event,
) {
    collect_events(host, |seats, events| {
        let owns = |pointer: &Pointer| pointer.relative.as_ref() == Some(&relative);
        if let Some(pointer) = pointer_matching(&mut seats.seats, owns) {
            pointer.relative_motion(event, events);
        }
    });
}

fn on_zwp_text_input_v3<D: SeatHost>(
    host: &mut D,
    input: ZwpTextInputV3,
    event: zwp_text_input_v3::Event,
) {
    collect_events(host, |seats, events| {
        if let Some(text_input) = seats
            .seats
            .iter_mut()
            .filter_map(|seat| seat.text_input.as_mut())
            .find(|text_input| text_input.input == input)
        {
            text_input.handle(event, events);
        }
    });
}

fn on_zwp_tablet_seat_v2<D: SeatHost>(
    host: &mut D,
    tablet_seat: ZwpTabletSeatV2,
    event: zwp_tablet_seat_v2::Event,
) {
    let Seats {
        seats, managers, ..
    } = host.seats();
    if let Some(tablet) = seats
        .iter_mut()
        .filter_map(|seat| seat.tablet.as_mut())
        .find(|tablet| tablet.is(&tablet_seat))
    {
        tablet.added(event, &managers.cursors);
    }
}

fn on_zwp_tablet_v2<D: SeatHost>(host: &mut D, tablet: ZwpTabletV2, event: zwp_tablet_v2::Event) {
    if let Some(owner) = host
        .seats()
        .seats
        .iter_mut()
        .filter_map(|seat| seat.tablet.as_mut())
        .find(|owner| owner.tablets.iter().any(|(owned, _)| *owned == tablet))
    {
        owner.describe(&tablet, event);
    }
}

fn on_zwp_tablet_tool_v2<D: SeatHost>(
    host: &mut D,
    tool: ZwpTabletToolV2,
    event: zwp_tablet_tool_v2::Event,
) {
    collect_tablet_events(host, |seats, events| {
        let Some((at, index)) = seats.seats.iter().enumerate().find_map(|(at, seat)| {
            seat.tablet
                .as_ref()?
                .tools
                .iter()
                .position(|owned| owned.proxy == tool)
                .map(|index| (at, index))
        }) else {
            return;
        };
        if seats.seats[at]
            .tablet
            .as_ref()
            .is_some_and(|tablet| tablet.tools[index].completes_discrete_frame(&event))
        {
            seats.flush_tablets(events);
        }
        let seat = &mut seats.seats[at];
        let modifiers = seat.modifiers;
        let wl_seat = seat.seat.clone();
        let Some(tablet) = seat.tablet.as_mut() else {
            return;
        };
        if matches!(event, zwp_tablet_tool_v2::Event::Removed) {
            tablet.remove_tool(index, modifiers, events);
            return;
        }
        let stroke = tablet.tools[index].handle(event, modifiers, events);
        if let Some((kind, serial)) = stroke {
            if kind == SerialKind::ButtonPressed {
                seats.pressed(&wl_seat, serial);
            }
            seats.record(&wl_seat, Some((kind, serial)));
        }
    });
}

fn pad_matching(seats: &mut [Seat], owns: impl Fn(&PadDevice) -> bool) -> Option<&mut PadDevice> {
    seats
        .iter_mut()
        .filter_map(|seat| seat.tablet.as_mut())
        .flat_map(|tablet| tablet.pads.iter_mut())
        .find(|pad| owns(pad))
}

fn on_zwp_tablet_pad_v2<D: SeatHost>(
    host: &mut D,
    pad: ZwpTabletPadV2,
    event: zwp_tablet_pad_v2::Event,
) {
    collect_events(host, |seats, events| {
        if matches!(event, zwp_tablet_pad_v2::Event::Removed) {
            for tablet in seats
                .seats
                .iter_mut()
                .filter_map(|seat| seat.tablet.as_mut())
            {
                if let Some(at) = tablet.pads.iter().position(|owned| owned.proxy == pad) {
                    tablet.remove_pad(at, events);
                    return;
                }
            }
            return;
        }
        if let Some(owner) = pad_matching(&mut seats.seats, |owned| owned.proxy == pad) {
            owner.handle(event, events);
        }
    });
}

fn on_zwp_tablet_pad_group_v2<D: SeatHost>(
    host: &mut D,
    group: ZwpTabletPadGroupV2,
    event: zwp_tablet_pad_group_v2::Event,
) {
    collect_events(host, |seats, events| {
        if let Some(owner) = pad_matching(&mut seats.seats, |pad| pad.owns_group(&group)) {
            owner.handle_group(&group, event, events);
        }
    });
}

fn on_zwp_tablet_pad_ring_v2<D: SeatHost>(
    host: &mut D,
    ring: ZwpTabletPadRingV2,
    event: zwp_tablet_pad_ring_v2::Event,
) {
    collect_events(host, |seats, events| {
        if let Some(owner) = pad_matching(&mut seats.seats, |pad| pad.owns_ring(&ring)) {
            owner.handle_ring(&ring, event, events);
        }
    });
}

fn on_zwp_tablet_pad_strip_v2<D: SeatHost>(
    host: &mut D,
    strip: ZwpTabletPadStripV2,
    event: zwp_tablet_pad_strip_v2::Event,
) {
    collect_events(host, |seats, events| {
        if let Some(owner) = pad_matching(&mut seats.seats, |pad| pad.owns_strip(&strip)) {
            owner.handle_strip(&strip, event, events);
        }
    });
}

fn on_zwp_tablet_pad_dial_v2<D: SeatHost>(
    host: &mut D,
    dial: ZwpTabletPadDialV2,
    event: zwp_tablet_pad_dial_v2::Event,
) {
    collect_events(host, |seats, events| {
        if let Some(owner) = pad_matching(&mut seats.seats, |pad| pad.owns_dial(&dial)) {
            owner.handle_dial(&dial, event, events);
        }
    });
}

fn keyboard_stroke(event: &wl_keyboard::Event) -> Option<(SerialKind, u32)> {
    match event {
        wl_keyboard::Event::Enter { serial, .. } => Some((SerialKind::KeyboardEntered, *serial)),
        wl_keyboard::Event::Leave { serial, .. } => Some((SerialKind::KeyboardLeft, *serial)),
        wl_keyboard::Event::Key {
            serial,
            state: Ok(KeyState::Released),
            ..
        } => Some((SerialKind::KeyReleased, *serial)),
        wl_keyboard::Event::Key { serial, .. } => Some((SerialKind::KeyPressed, *serial)),
        wl_keyboard::Event::Modifiers { serial, .. } => {
            Some((SerialKind::ModifiersChanged, *serial))
        }
        _ => None,
    }
}

fn pointer_stroke(event: &wl_pointer::Event) -> Option<(SerialKind, u32)> {
    match event {
        wl_pointer::Event::Enter { serial, .. } => Some((SerialKind::PointerEntered, *serial)),
        wl_pointer::Event::Leave { serial, .. } => Some((SerialKind::PointerLeft, *serial)),
        wl_pointer::Event::Button {
            serial,
            state: Ok(ButtonState::Pressed),
            ..
        } => Some((SerialKind::ButtonPressed, *serial)),
        wl_pointer::Event::Button { serial, .. } => Some((SerialKind::ButtonReleased, *serial)),
        _ => None,
    }
}
