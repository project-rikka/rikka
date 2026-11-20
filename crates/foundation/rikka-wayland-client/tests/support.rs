use std::os::fd::AsFd;

use rikka_compositor_protocols::wl_buffer::WlBuffer;
use rikka_compositor_protocols::wl_compositor::{self, WlCompositor};
use rikka_compositor_protocols::wl_data_device::WlDataDevice;
use rikka_compositor_protocols::wl_data_device_manager::{self, WlDataDeviceManager};
use rikka_compositor_protocols::wl_data_offer::{self, WlDataOffer};
use rikka_compositor_protocols::wl_keyboard::{self, WlKeyboard};
use rikka_compositor_protocols::wl_output::WlOutput;
use rikka_compositor_protocols::wl_pointer::WlPointer;
use rikka_compositor_protocols::wl_region::WlRegion;
use rikka_compositor_protocols::wl_seat::{self, WlSeat};
use rikka_compositor_protocols::wl_shm::{self, WlShm};
use rikka_compositor_protocols::wl_shm_pool::WlShmPool;
use rikka_compositor_protocols::wl_surface::{self, WlSurface};
use rikka_compositor_server::{Client, DisplayHandle};
use rikka_test_display::{Display, Host, Notes};
use rustix::fs::{MemfdFlags, memfd_create};

pub(crate) struct State {
    display: DisplayHandle<State>,
    notes: Notes,
    surface: Option<WlSurface>,
}

impl Host for State {
    fn display_handle(&self) -> &DisplayHandle<Self> {
        &self.display
    }
}

pub(crate) fn marked_memfd(marker: u8) -> std::os::fd::OwnedFd {
    let fd = memfd_create("wayland-client-test", MemfdFlags::CLOEXEC).unwrap();
    rustix::io::write(&fd, &[marker]).unwrap();
    fd
}

pub(crate) fn marker(fd: impl AsFd) -> u8 {
    let mut byte = [0u8];
    rustix::io::pread(fd, &mut byte, 0).unwrap();
    byte[0]
}

pub(crate) fn start() -> Display<State> {
    Display::start(|reactor, notes| {
        let display = DisplayHandle::new(reactor);
        display.serve::<WlCompositor>(compositor);
        display
            .serve::<WlSurface>(surface)
            .destroyed(surface_destroyed);
        display.serve::<WlRegion>(|_, _, _| {});
        display
            .serve::<WlShm>(shm)
            .bound(|_: &mut State, shm: WlShm| {
                shm.format(wl_shm::Format::Argb8888);
            });
        display.serve::<WlShmPool>(|_, _, _| {});
        display.serve::<WlBuffer>(|_, _, _| {});
        display
            .serve::<WlSeat>(seat)
            .bound(|_: &mut State, seat: WlSeat| {
                seat.capabilities(wl_seat::Capability::KEYBOARD | wl_seat::Capability::POINTER);
                seat.name("seat-test");
            });
        display.serve::<WlKeyboard>(|_, _, _| {});
        display.serve::<WlPointer>(|_, _, _| {});
        display.serve::<WlDataDeviceManager>(data_device_manager);
        display.serve::<WlDataDevice>(|_, _, _| {});
        display.serve::<WlDataOffer>(data_offer);
        display
            .serve::<WlOutput>(|_, _, _| {})
            .bound(|_: &mut State, output: WlOutput| {
                output.name(match output.key::<u32>() {
                    Some(1) => "DP-1",
                    _ => "HDMI-A-1",
                });
                output.done();
            });
        display.global::<WlCompositor>().publish();
        display.global::<WlShm>().version(1).publish();
        display.global::<WlSeat>().version(7).publish();
        display.global::<WlDataDeviceManager>().publish();
        display.global::<WlOutput>().key(1u32).publish();
        display.global::<WlOutput>().key(2u32).publish();
        State {
            display,
            notes,
            surface: None,
        }
    })
}

fn compositor(state: &mut State, _: WlCompositor, request: wl_compositor::Request) {
    if let wl_compositor::Request::CreateSurface { id } = request {
        state.notes.note(format!("surface {}", id.id()));
        state.surface = Some(id);
    }
}

fn surface(state: &mut State, surface: WlSurface, request: wl_surface::Request) {
    match request {
        wl_surface::Request::Attach { buffer, x, y } => state.notes.note(format!(
            "attach {:?} {x} {y}",
            buffer.map(|buffer| buffer.id())
        )),
        wl_surface::Request::SetBufferScale { scale: 0 } => {
            surface.post_error(wl_surface::Error::InvalidScale);
        }
        wl_surface::Request::SetBufferScale { scale } => state.notes.note(format!("scale {scale}")),
        wl_surface::Request::Frame { callback } => callback.done(7),
        _ => {}
    }
}

fn surface_destroyed(state: &mut State, surface: WlSurface) {
    state
        .notes
        .note(format!("destroyed surface {}", surface.id()));
}

fn shm(state: &mut State, _: WlShm, request: wl_shm::Request) {
    if let wl_shm::Request::CreatePool { fd, size, .. } = request {
        state
            .notes
            .note(format!("pool {size} marked {}", marker(&fd)));
    }
}

fn seat(state: &mut State, _: WlSeat, request: wl_seat::Request) {
    let Some(surface) = state.surface.clone() else {
        return;
    };
    match request {
        wl_seat::Request::GetKeyboard { id } => {
            id.keymap(wl_keyboard::KeymapFormat::XkbV1, marked_memfd(42), 1);
            id.enter(3, &surface, &[30, 0, 0, 0, 31, 0, 0, 0, 9]);
        }
        wl_seat::Request::GetPointer { id } => id.enter(4, &surface, 1.5, -2.25),
        _ => {}
    }
}

fn data_device_manager(
    state: &mut State,
    _: WlDataDeviceManager,
    request: wl_data_device_manager::Request,
) {
    if let wl_data_device_manager::Request::GetDataDevice { id, .. } = request {
        let Some(client) = Client::from_resource(&id) else {
            return;
        };
        let offer: WlDataOffer = client.create(id.version());
        id.data_offer(&offer);
        offer.offer("text/plain");
        id.selection(Some(&offer));
        id.selection(None);
        state.notes.note("offered");
    }
}

fn data_offer(state: &mut State, _: WlDataOffer, request: wl_data_offer::Request<'_>) {
    if let wl_data_offer::Request::Accept { serial, mime_type } = request {
        state.notes.note(format!("accept {serial} {mime_type:?}"));
    }
}
