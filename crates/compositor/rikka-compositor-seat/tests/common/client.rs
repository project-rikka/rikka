use std::ops::{Deref, DerefMut};
use std::os::fd::OwnedFd;
use std::os::unix::fs::FileExt;
use std::os::unix::net::UnixStream;
use std::time::Instant;

use rikka_test_display::{Canvas, Client};
use rikka_wayland_client::{
    Queue, ext_idle_notification_v1, wl_compositor, wl_keyboard, wl_pointer, wl_seat, wl_shm,
    wl_surface, wl_touch, zwp_confined_pointer_v1, zwp_input_method_keyboard_grab_v2,
    zwp_input_method_v2, zwp_input_popup_surface_v2, zwp_keyboard_shortcuts_inhibitor_v1,
    zwp_locked_pointer_v1, zwp_pointer_gesture_hold_v1, zwp_pointer_gesture_pinch_v1,
    zwp_pointer_gesture_swipe_v1, zwp_relative_pointer_v1, zwp_tablet_pad_group_v2,
    zwp_tablet_pad_v2, zwp_tablet_seat_v2, zwp_tablet_tool_v2, zwp_tablet_v2, zwp_text_input_v3,
};
use rustix::event::{PollFd, PollFlags, poll};
use rustix::fs::{MemfdFlags, ftruncate, memfd_create};
use rustix::time::Timespec;

#[derive(Debug, Default)]
pub(crate) struct Received {
    pub(crate) seat: Vec<(u32, wl_seat::Event)>,
    pub(crate) keyboard: Vec<(u32, wl_keyboard::Event)>,
    pub(crate) keymaps: Vec<String>,
    pub(crate) pointer: Vec<wl_pointer::Event>,
    pub(crate) touch: Vec<wl_touch::Event>,
    pub(crate) relative: Vec<zwp_relative_pointer_v1::Event>,
    pub(crate) locked: Vec<zwp_locked_pointer_v1::Event>,
    pub(crate) confined: Vec<zwp_confined_pointer_v1::Event>,
    pub(crate) swipe: Vec<zwp_pointer_gesture_swipe_v1::Event>,
    pub(crate) pinch: Vec<zwp_pointer_gesture_pinch_v1::Event>,
    pub(crate) hold: Vec<zwp_pointer_gesture_hold_v1::Event>,
    pub(crate) inhibitor: Vec<zwp_keyboard_shortcuts_inhibitor_v1::Event>,
    pub(crate) tablet_seat: Vec<zwp_tablet_seat_v2::Event>,
    pub(crate) tablet: Vec<zwp_tablet_v2::Event>,
    pub(crate) tool: Vec<zwp_tablet_tool_v2::Event>,
    pub(crate) pad: Vec<zwp_tablet_pad_v2::Event>,
    pub(crate) pad_group: Vec<zwp_tablet_pad_group_v2::Event>,
    pub(crate) text_input: Vec<zwp_text_input_v3::Event>,
    pub(crate) input_method: Vec<zwp_input_method_v2::Event>,
    pub(crate) grab: Vec<zwp_input_method_keyboard_grab_v2::Event>,
    pub(crate) popup: Vec<zwp_input_popup_surface_v2::Event>,
    pub(crate) idle: Vec<ext_idle_notification_v1::Event>,
}

impl Received {
    pub(crate) fn keys(&self) -> Vec<(u32, wl_keyboard::KeyState)> {
        self.keyboard
            .iter()
            .filter_map(|(_, event)| match event {
                wl_keyboard::Event::Key {
                    key,
                    state: Ok(state),
                    ..
                } => Some((*key, *state)),
                _ => None,
            })
            .collect()
    }

    pub(crate) fn grabbed_keys(&self) -> Vec<(u32, wl_keyboard::KeyState)> {
        self.grab
            .iter()
            .filter_map(|event| match event {
                zwp_input_method_keyboard_grab_v2::Event::Key {
                    key,
                    state: Ok(state),
                    ..
                } => Some((*key, *state)),
                _ => None,
            })
            .collect()
    }
}

fn on_wl_seat(received: &mut Received, proxy: wl_seat::WlSeat, event: wl_seat::Event) {
    let tag = &proxy.key::<u32>().expect("the object is tagged");
    received.seat.push((*tag, event));
}

fn on_wl_keyboard(
    received: &mut Received,
    proxy: wl_keyboard::WlKeyboard,
    event: wl_keyboard::Event,
) {
    let tag = &proxy.key::<u32>().expect("the object is tagged");
    if let wl_keyboard::Event::Keymap { fd, size, .. } = &event {
        received.keymaps.push(read_keymap(fd, *size));
    }
    received.keyboard.push((*tag, event));
}

fn on_zwp_input_method_keyboard_grab_v2(
    received: &mut Received,
    _: zwp_input_method_keyboard_grab_v2::ZwpInputMethodKeyboardGrabV2,
    event: zwp_input_method_keyboard_grab_v2::Event,
) {
    if let zwp_input_method_keyboard_grab_v2::Event::Keymap { fd, size, .. } = &event {
        received.keymaps.push(read_keymap(fd, *size));
    }
    received.grab.push(event);
}

fn on_zwp_tablet_seat_v2(
    received: &mut Received,
    _: zwp_tablet_seat_v2::ZwpTabletSeatV2,
    event: zwp_tablet_seat_v2::Event,
) {
    received.tablet_seat.push(event);
}

fn on_zwp_tablet_pad_v2(
    received: &mut Received,
    _: zwp_tablet_pad_v2::ZwpTabletPadV2,
    event: zwp_tablet_pad_v2::Event,
) {
    received.pad.push(event);
}

fn on_zwp_tablet_pad_group_v2(
    received: &mut Received,
    _: zwp_tablet_pad_group_v2::ZwpTabletPadGroupV2,
    event: zwp_tablet_pad_group_v2::Event,
) {
    received.pad_group.push(event);
}

fn read_keymap(fd: &OwnedFd, size: u32) -> String {
    let file = std::fs::File::from(fd.try_clone().unwrap());
    let mut bytes = vec![0; size as usize];
    file.read_exact_at(&mut bytes, 0).unwrap();
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8(bytes[..end].to_vec()).unwrap()
}

pub(crate) fn keymap_file(text: &str) -> (OwnedFd, u32) {
    let memory = memfd_create("seat-test-keymap", MemfdFlags::CLOEXEC).unwrap();
    let mut bytes = text.as_bytes().to_vec();
    bytes.push(0);
    ftruncate(&memory, bytes.len() as u64).unwrap();
    rustix::io::write(&memory, &bytes).unwrap();
    let size = bytes.len() as u32;
    (memory, size)
}

pub(crate) struct Mapped {
    pub(crate) surface: wl_surface::WlSurface,
    pub(crate) id: u32,
    _canvas: Canvas,
}

pub(crate) struct Wayland(Client<Received>);

impl Deref for Wayland {
    type Target = Client<Received>;

    fn deref(&self) -> &Client<Received> {
        &self.0
    }
}

impl DerefMut for Wayland {
    fn deref_mut(&mut self) -> &mut Client<Received> {
        &mut self.0
    }
}

fn listen(queue: &mut Queue<Received>) {
    queue.on::<wl_seat::WlSeat>(on_wl_seat);
    queue.on::<wl_keyboard::WlKeyboard>(on_wl_keyboard);
    queue.on::<zwp_input_method_keyboard_grab_v2::ZwpInputMethodKeyboardGrabV2>(
        on_zwp_input_method_keyboard_grab_v2,
    );
    queue.on::<zwp_tablet_seat_v2::ZwpTabletSeatV2>(on_zwp_tablet_seat_v2);
    queue.on::<zwp_tablet_pad_v2::ZwpTabletPadV2>(on_zwp_tablet_pad_v2);
    queue.on::<zwp_tablet_pad_group_v2::ZwpTabletPadGroupV2>(on_zwp_tablet_pad_group_v2);
    queue.on::<wl_pointer::WlPointer>(|received: &mut Received, _, event| {
        received.pointer.push(event)
    });
    queue.on::<wl_touch::WlTouch>(|received: &mut Received, _, event| received.touch.push(event));
    queue.on::<zwp_relative_pointer_v1::ZwpRelativePointerV1>(
        |received: &mut Received, _, event| received.relative.push(event),
    );
    queue.on::<zwp_locked_pointer_v1::ZwpLockedPointerV1>(|received: &mut Received, _, event| {
        received.locked.push(event)
    });
    queue.on::<zwp_confined_pointer_v1::ZwpConfinedPointerV1>(
        |received: &mut Received, _, event| received.confined.push(event),
    );
    queue.on::<zwp_pointer_gesture_swipe_v1::ZwpPointerGestureSwipeV1>(
        |received: &mut Received, _, event| received.swipe.push(event),
    );
    queue.on::<zwp_pointer_gesture_pinch_v1::ZwpPointerGesturePinchV1>(
        |received: &mut Received, _, event| received.pinch.push(event),
    );
    queue.on::<zwp_pointer_gesture_hold_v1::ZwpPointerGestureHoldV1>(
        |received: &mut Received, _, event| received.hold.push(event),
    );
    queue.on::<zwp_keyboard_shortcuts_inhibitor_v1::ZwpKeyboardShortcutsInhibitorV1>(
        |received: &mut Received, _, event| received.inhibitor.push(event),
    );
    queue.on::<zwp_tablet_v2::ZwpTabletV2>(|received: &mut Received, _, event| {
        received.tablet.push(event)
    });
    queue.on::<zwp_tablet_tool_v2::ZwpTabletToolV2>(|received: &mut Received, _, event| {
        received.tool.push(event)
    });
    queue.on::<zwp_text_input_v3::ZwpTextInputV3>(|received: &mut Received, _, event| {
        received.text_input.push(event)
    });
    queue.on::<zwp_input_method_v2::ZwpInputMethodV2>(|received: &mut Received, _, event| {
        received.input_method.push(event)
    });
    queue.on::<zwp_input_popup_surface_v2::ZwpInputPopupSurfaceV2>(
        |received: &mut Received, _, event| received.popup.push(event),
    );
    queue.on::<ext_idle_notification_v1::ExtIdleNotificationV1>(
        |received: &mut Received, _, event| received.idle.push(event),
    );
}

impl Wayland {
    pub(crate) fn new(stream: UnixStream) -> Self {
        Self(Client::connect(stream, listen))
    }

    pub(crate) fn wait_readable(&mut self, deadline: Instant) -> bool {
        self.connection.flush().unwrap();
        let left = deadline.saturating_duration_since(Instant::now());
        let timeout = Timespec {
            tv_sec: left.as_secs() as i64,
            tv_nsec: i64::from(left.subsec_nanos()),
        };
        let mut fds = [PollFd::new(&self.connection, PollFlags::IN)];
        poll(&mut fds, Some(&timeout)).unwrap() > 0
    }

    pub(crate) fn seats(&self) -> Vec<(u32, String)> {
        self.globals()
            .into_iter()
            .filter(|global| &*global.interface == "wl_seat")
            .map(|global| (global.name, global.interface.into()))
            .collect()
    }

    pub(crate) fn seat(&self, index: usize) -> wl_seat::WlSeat {
        self.seat_at(index, 11)
    }

    pub(crate) fn seat_at(&self, index: usize, version: u32) -> wl_seat::WlSeat {
        let (name, _) = self.seats()[index].clone();
        self.bind::<wl_seat::WlSeat>(name, version)
            .with_key(index as u32)
    }

    pub(crate) fn mapped(&mut self, width: i32, height: i32) -> Mapped {
        let compositor: wl_compositor::WlCompositor = self.global(6);
        let surface = compositor.create_surface();
        let canvas = self.canvas(width, height, wl_shm::Format::Argb8888);
        surface.attach(Some(&canvas.buffer), 0, 0);
        surface.commit();
        self.roundtrip();
        let id = surface.id();
        Mapped {
            surface,
            id,
            _canvas: canvas,
        }
    }
}
