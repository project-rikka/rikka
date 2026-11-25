use std::io::Write as _;
use std::os::fd::AsFd;

use rikka_wayland_client::{ext_idle_notification_v1, ext_idle_notifier_v1};
use rikka_wayland_client::{wl_keyboard, wl_pointer, wl_seat, wl_surface};
use rikka_wayland_client::{zwlr_virtual_pointer_manager_v1, zwlr_virtual_pointer_v1};
use rikka_wayland_client::{zwp_idle_inhibit_manager_v1, zwp_idle_inhibitor_v1};
use rikka_wayland_client::{
    zwp_input_method_keyboard_grab_v2, zwp_input_method_manager_v2, zwp_input_method_v2,
};
use rikka_wayland_client::{
    zwp_keyboard_shortcuts_inhibit_manager_v1, zwp_keyboard_shortcuts_inhibitor_v1,
};
use rikka_wayland_client::{zwp_locked_pointer_v1, zwp_pointer_constraints_v1};
use rikka_wayland_client::{zwp_relative_pointer_manager_v1, zwp_relative_pointer_v1};
use rikka_wayland_client::{zwp_text_input_manager_v3, zwp_text_input_v3};
use rikka_wayland_client::{zwp_virtual_keyboard_manager_v1, zwp_virtual_keyboard_v1};

use super::client::{Client, Recorded};

pub(crate) const A: u32 = 30;
pub(crate) const Q: u32 = 16;
pub(crate) const P: u32 = 25;
pub(crate) const SUPER: u32 = 125;
pub(crate) const PRESSED: u32 = 1;
pub(crate) const RELEASED: u32 = 0;

const XKB_V1: u32 = 1;

pub(crate) const KEYMAP: &str = r#"xkb_keymap {
    xkb_keycodes "rikka" {
        minimum = 8;
        maximum = 255;
        <AD01> = 24;
        <AC01> = 38;
        <AD10> = 33;
        <LWIN> = 133;
        <TAB> = 23;
        <LALT> = 64;
    };
    xkb_types "rikka" {
        type "ONE_LEVEL" {
            modifiers = none;
            level_name[Level1] = "Any";
        };
    };
    xkb_compatibility "rikka" {
        interpret Super_L { action = SetMods(modifiers = Mod4); };
        interpret Alt_L { action = SetMods(modifiers = Mod1); };
    };
    xkb_symbols "rikka" {
        key <AD01> { [ q ] };
        key <AC01> { [ a ] };
        key <AD10> { [ p ] };
        key <LWIN> { [ Super_L ] };
        key <TAB> { [ Tab ] };
        key <LALT> { [ Alt_L ] };
        modifier_map Mod4 { <LWIN> };
        modifier_map Mod1 { <LALT> };
    };
};
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Typed {
    Entered,
    Left,
    Committed(String),
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Composed {
    Activated,
    Deactivated,
    Done,
}

#[derive(Debug, Default)]
pub(crate) struct InputRecord {
    pub(crate) seats: Vec<(u32, String)>,
    pub(crate) entered_at: Option<(f64, f64)>,
    pub(crate) typed: Vec<Typed>,
    pub(crate) composed: Vec<Composed>,
    pub(crate) grabbed: Vec<u32>,
    pub(crate) locked: Vec<bool>,
    pub(crate) travelled: Vec<(f64, f64)>,
    pub(crate) idle: Vec<bool>,
}

pub(crate) struct Seated {
    pub(crate) name: String,
    pub(crate) seat: wl_seat::WlSeat,
}

impl Client {
    pub(crate) fn seats(&mut self) -> Vec<Seated> {
        let offered: Vec<(u32, u32)> = self
            .globals()
            .iter()
            .filter(|global| &*global.interface == "wl_seat")
            .map(|global| (global.name, global.version.min(7)))
            .collect();
        let seats: Vec<(u32, wl_seat::WlSeat)> = offered
            .into_iter()
            .map(|(name, version)| (name, self.bind_seat(name, version)))
            .collect();
        self.roundtrip();
        seats
            .into_iter()
            .map(|(tag, seat)| Seated {
                name: self
                    .received
                    .input
                    .seats
                    .iter()
                    .find(|(seat_tag, _)| *seat_tag == tag)
                    .map(|(_, name)| name.clone())
                    .unwrap_or_default(),
                seat,
            })
            .collect()
    }

    pub(crate) fn located_pointer(&mut self, seat: &wl_seat::WlSeat) -> wl_pointer::WlPointer {
        let pointer = seat.get_pointer().with_key(0u32);
        self.roundtrip();
        pointer
    }

    pub(crate) fn virtual_keyboard(
        &mut self,
        seat: &wl_seat::WlSeat,
    ) -> zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1 {
        let manager: zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1 = self.global(1);
        let keyboard = manager.create_virtual_keyboard(seat);
        let (file, size) = keymap_file();
        keyboard.keymap(XKB_V1, file.as_fd(), size);
        self.roundtrip();
        keyboard
    }

    pub(crate) fn inhibit_shortcuts(
        &mut self,
        surface: &wl_surface::WlSurface,
        seat: &wl_seat::WlSeat,
    ) -> zwp_keyboard_shortcuts_inhibitor_v1::ZwpKeyboardShortcutsInhibitorV1 {
        let manager: zwp_keyboard_shortcuts_inhibit_manager_v1::ZwpKeyboardShortcutsInhibitManagerV1 =
            self.global(1);
        let inhibitor = manager.inhibit_shortcuts(surface, seat);
        self.roundtrip();
        inhibitor
    }

    pub(crate) fn virtual_pointer(
        &mut self,
        seat: Option<&wl_seat::WlSeat>,
    ) -> zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1 {
        let manager: zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1 = self.global(2);
        let pointer = manager.create_virtual_pointer(seat);
        self.roundtrip();
        pointer
    }

    pub(crate) fn text_input(
        &mut self,
        seat: &wl_seat::WlSeat,
    ) -> zwp_text_input_v3::ZwpTextInputV3 {
        let manager: zwp_text_input_manager_v3::ZwpTextInputManagerV3 = self.global(1);
        let text_input = manager.get_text_input(seat);
        self.roundtrip();
        text_input
    }

    pub(crate) fn input_method(
        &mut self,
        seat: &wl_seat::WlSeat,
    ) -> (
        zwp_input_method_v2::ZwpInputMethodV2,
        zwp_input_method_keyboard_grab_v2::ZwpInputMethodKeyboardGrabV2,
    ) {
        let manager: zwp_input_method_manager_v2::ZwpInputMethodManagerV2 = self.global(1);
        let method = manager.get_input_method(seat);
        let grab = method.grab_keyboard();
        self.roundtrip();
        (method, grab)
    }

    pub(crate) fn lock_pointer(
        &mut self,
        surface: &wl_surface::WlSurface,
        pointer: &wl_pointer::WlPointer,
    ) -> zwp_locked_pointer_v1::ZwpLockedPointerV1 {
        let constraints: zwp_pointer_constraints_v1::ZwpPointerConstraintsV1 = self.global(1);
        let locked = constraints.lock_pointer(
            surface,
            pointer,
            None,
            zwp_pointer_constraints_v1::Lifetime::Persistent,
        );
        self.roundtrip();
        locked
    }

    pub(crate) fn relative_pointer(
        &mut self,
        pointer: &wl_pointer::WlPointer,
    ) -> zwp_relative_pointer_v1::ZwpRelativePointerV1 {
        let manager: zwp_relative_pointer_manager_v1::ZwpRelativePointerManagerV1 = self.global(1);
        let relative = manager.get_relative_pointer(pointer);
        self.roundtrip();
        relative
    }

    pub(crate) fn inhibit_idle(
        &mut self,
        surface: &wl_surface::WlSurface,
    ) -> zwp_idle_inhibitor_v1::ZwpIdleInhibitorV1 {
        let manager: zwp_idle_inhibit_manager_v1::ZwpIdleInhibitManagerV1 = self.global(1);
        let inhibitor = manager.create_inhibitor(surface);
        self.roundtrip();
        inhibitor
    }

    pub(crate) fn idle_after(
        &mut self,
        seat: &wl_seat::WlSeat,
        millis: u32,
    ) -> ext_idle_notification_v1::ExtIdleNotificationV1 {
        let notifier: ext_idle_notifier_v1::ExtIdleNotifierV1 = self.global(1);
        let notification = notifier.get_idle_notification(millis, seat);
        self.roundtrip();
        notification
    }

    fn bind_seat(&self, name: u32, version: u32) -> wl_seat::WlSeat {
        self.bind::<wl_seat::WlSeat>(name, version).with_key(name)
    }
}

fn keymap_file() -> (std::fs::File, u32) {
    let mut file = tempfile::tempfile().unwrap();
    file.write_all(KEYMAP.as_bytes()).unwrap();
    file.write_all(&[0]).unwrap();
    (file, KEYMAP.len() as u32 + 1)
}

fn on_wl_seat(recorded: &mut Recorded, proxy: wl_seat::WlSeat, event: wl_seat::Event) {
    let Some(tag) = proxy.key::<u32>() else {
        return;
    };
    if let wl_seat::Event::Name { name } = event {
        recorded.input.seats.push((tag, name));
    }
}

pub(super) fn tagged_pointer(recorded: &mut Recorded, event: wl_pointer::Event) {
    if let wl_pointer::Event::Enter {
        surface_x,
        surface_y,
        ..
    } = event
    {
        recorded.input.entered_at = Some((surface_x, surface_y));
    }
}

fn on_zwp_text_input_v3(
    recorded: &mut Recorded,
    _: zwp_text_input_v3::ZwpTextInputV3,
    event: zwp_text_input_v3::Event,
) {
    let typed = match event {
        zwp_text_input_v3::Event::Enter { .. } => Typed::Entered,
        zwp_text_input_v3::Event::Leave { .. } => Typed::Left,
        zwp_text_input_v3::Event::CommitString { text } => {
            Typed::Committed(text.unwrap_or_default())
        }
        zwp_text_input_v3::Event::Done { .. } => Typed::Done,
        _ => return,
    };
    recorded.input.typed.push(typed);
}

fn on_zwp_input_method_v2(
    recorded: &mut Recorded,
    _: zwp_input_method_v2::ZwpInputMethodV2,
    event: zwp_input_method_v2::Event,
) {
    let composed = match event {
        zwp_input_method_v2::Event::Activate => Composed::Activated,
        zwp_input_method_v2::Event::Deactivate => Composed::Deactivated,
        zwp_input_method_v2::Event::Done => Composed::Done,
        _ => return,
    };
    recorded.input.composed.push(composed);
}

fn on_zwp_input_method_keyboard_grab_v2(
    recorded: &mut Recorded,
    _: zwp_input_method_keyboard_grab_v2::ZwpInputMethodKeyboardGrabV2,
    event: zwp_input_method_keyboard_grab_v2::Event,
) {
    if let zwp_input_method_keyboard_grab_v2::Event::Key {
        key,
        state: Ok(wl_keyboard::KeyState::Pressed),
        ..
    } = event
    {
        recorded.input.grabbed.push(key);
    }
}

fn on_zwp_locked_pointer_v1(
    recorded: &mut Recorded,
    _: zwp_locked_pointer_v1::ZwpLockedPointerV1,
    event: zwp_locked_pointer_v1::Event,
) {
    match event {
        zwp_locked_pointer_v1::Event::Locked => recorded.input.locked.push(true),
        zwp_locked_pointer_v1::Event::Unlocked => recorded.input.locked.push(false),
        _ => {}
    }
}

fn on_zwp_relative_pointer_v1(
    recorded: &mut Recorded,
    _: zwp_relative_pointer_v1::ZwpRelativePointerV1,
    event: zwp_relative_pointer_v1::Event,
) {
    if let zwp_relative_pointer_v1::Event::RelativeMotion { dx, dy, .. } = event {
        recorded.input.travelled.push((dx, dy));
    }
}

fn on_ext_idle_notification_v1(
    recorded: &mut Recorded,
    _: ext_idle_notification_v1::ExtIdleNotificationV1,
    event: ext_idle_notification_v1::Event,
) {
    match event {
        ext_idle_notification_v1::Event::Idled => recorded.input.idle.push(true),
        ext_idle_notification_v1::Event::Resumed => recorded.input.idle.push(false),
        _ => {}
    }
}

pub(crate) fn listen(queue: &mut rikka_wayland_client::Queue<Recorded>) {
    queue.on::<wl_seat::WlSeat>(on_wl_seat);
    queue.on::<zwp_text_input_v3::ZwpTextInputV3>(on_zwp_text_input_v3);
    queue.on::<zwp_input_method_v2::ZwpInputMethodV2>(on_zwp_input_method_v2);
    queue.on::<zwp_input_method_keyboard_grab_v2::ZwpInputMethodKeyboardGrabV2>(
        on_zwp_input_method_keyboard_grab_v2,
    );
    queue.on::<zwp_locked_pointer_v1::ZwpLockedPointerV1>(on_zwp_locked_pointer_v1);
    queue.on::<zwp_relative_pointer_v1::ZwpRelativePointerV1>(on_zwp_relative_pointer_v1);
    queue.on::<ext_idle_notification_v1::ExtIdleNotificationV1>(on_ext_idle_notification_v1);
}
