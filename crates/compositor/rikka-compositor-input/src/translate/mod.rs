mod gesture;
mod pointer;
mod tablet;
mod touch;

pub(crate) use tablet::Tools;

use input::event::Event;
use input::event::keyboard::{KeyState, KeyboardEvent, KeyboardEventTrait};
use input::event::pointer::ButtonState;
use input::event::switch::{Switch as LibinputSwitch, SwitchEvent, SwitchEventTrait, SwitchState};

use rikka_compositor_reactor::Timestamp;
use rikka_compositor_types::Press;

use crate::devices::TrackedDevice;
use crate::event::{InputEvent, Switch};

pub(crate) fn translate(
    event: Event,
    tracked: &TrackedDevice,
    tools: &mut Tools,
) -> Option<InputEvent> {
    let device = tracked.id;
    match event {
        Event::Keyboard(KeyboardEvent::Key(key)) => Some(InputEvent::Key {
            device,
            time: Timestamp::from_micros(key.time_usec()),
            code: key.key(),
            press: match key.key_state() {
                KeyState::Pressed => Press::Pressed,
                KeyState::Released => Press::Released,
            },
            seat_count: key.seat_key_count(),
        }),
        Event::Pointer(event) => pointer::translate(event, device, tracked.natural),
        Event::Gesture(event) => gesture::translate(event, device),
        Event::Touch(event) => touch::translate(event, device),
        Event::Tablet(event) => tablet::tool(event, device, tools),
        Event::TabletPad(event) => tablet::pad(event, device),
        Event::Switch(SwitchEvent::Toggle(toggle)) => Some(InputEvent::Switch {
            device,
            time: Timestamp::from_micros(toggle.time_usec()),
            switch: match toggle.switch()? {
                LibinputSwitch::Lid => Switch::Lid,
                LibinputSwitch::TabletMode => Switch::TabletMode,
                _ => return None,
            },
            on: toggle.switch_state() == SwitchState::On,
        }),
        _ => None,
    }
}

fn press(state: ButtonState) -> Press {
    match state {
        ButtonState::Pressed => Press::Pressed,
        ButtonState::Released => Press::Released,
    }
}
