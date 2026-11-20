use rikka_input::DeviceEvent;
use rikka_seat::{SeatEvent, SeatHost, Seats};
use rikka_wayland_client::wl_callback::{self, WlCallback};
use rikka_wayland_client::wl_output::{self, WlOutput};
use rikka_wayland_client::wl_registry::{self, WlRegistry};
use rikka_wayland_client::wl_seat::WlSeat;
use rikka_wayland_client::wl_surface::{self, WlSurface};
use rikka_wayland_client::wp_fractional_scale_v1::{self, WpFractionalScaleV1};
use rikka_wayland_client::xdg_surface::{self, XdgSurface};
use rikka_wayland_client::xdg_toplevel::{self, XdgToplevel};
use rikka_wayland_client::xdg_wm_base::{self, XdgWmBase};
use rikka_wayland_client::{Proxy as _, Queue};

use super::screens::Screens;
use super::window::Window;

#[derive(Debug)]
pub(super) enum Heard {
    Input(DeviceEvent),
    Unfocused,
}

#[derive(Debug)]
pub(super) struct Host {
    pub(super) seats: Seats,
    pub(super) screens: Screens,
    pub(super) window: Window,
    pub(super) heard: Vec<Heard>,
    pub(super) reshaped: bool,
    pub(super) frame_due: bool,
    pub(super) closed: bool,
}

impl Host {
    pub(super) fn new(seats: Seats, window: Window) -> Self {
        Self {
            seats,
            screens: Screens::default(),
            window,
            heard: Vec::new(),
            reshaped: false,
            frame_due: false,
            closed: false,
        }
    }

    pub(super) fn listen(queue: &mut Queue<Host>) {
        Seats::listen(queue);
        queue.on::<WlRegistry>(on_wl_registry);
        queue.on::<WlOutput>(on_wl_output);
        queue.on::<XdgWmBase>(on_xdg_wm_base);
        queue.on::<XdgSurface>(on_xdg_surface);
        queue.on::<XdgToplevel>(on_xdg_toplevel);
        queue.on::<WpFractionalScaleV1>(on_wp_fractional_scale_v1);
        queue.on::<WlSurface>(on_wl_surface);
        queue.on::<WlCallback>(on_wl_callback);
    }
}

impl SeatHost for Host {
    fn seats(&mut self) -> &mut Seats {
        &mut self.seats
    }

    fn handle_event(&mut self, event: SeatEvent) {
        match event {
            SeatEvent::Input { surface, event } | SeatEvent::Stylus { surface, event, .. }
                if &surface == self.window.surface() =>
            {
                self.heard.push(Heard::Input(event));
            }
            SeatEvent::Keyboard {
                surface,
                entered: false,
            } if &surface == self.window.surface() => self.heard.push(Heard::Unfocused),
            SeatEvent::Input { .. }
            | SeatEvent::Stylus { .. }
            | SeatEvent::Keyboard { .. }
            | SeatEvent::Pad { .. } => {}
        }
    }
}

fn on_wl_registry(host: &mut Host, registry: WlRegistry, event: wl_registry::Event) {
    match event {
        wl_registry::Event::Global {
            name,
            interface,
            version,
        } => {
            if interface == WlOutput::INTERFACE.name {
                host.screens.add(&registry, name, version);
            } else if interface == WlSeat::INTERFACE.name {
                host.seats.add(&registry, name, version);
            }
        }
        wl_registry::Event::GlobalRemove { name } => match host.screens.remove(name) {
            Some(output) => {
                host.window.left(&output);
                host.reshaped = true;
            }
            None => {
                host.seats.remove(name);
            }
        },
        _ => {}
    }
}

fn on_wl_output(host: &mut Host, output: WlOutput, event: wl_output::Event) {
    host.reshaped |= host.screens.handle_event(&output, event);
}

fn on_xdg_wm_base(_: &mut Host, wm_base: XdgWmBase, event: xdg_wm_base::Event) {
    if let xdg_wm_base::Event::Ping { serial } = event {
        wm_base.pong(serial);
    }
}

fn on_xdg_surface(host: &mut Host, _: XdgSurface, event: xdg_surface::Event) {
    if let xdg_surface::Event::Configure { serial } = event {
        host.window.configured(serial);
        host.reshaped = true;
    }
}

fn on_xdg_toplevel(host: &mut Host, _: XdgToplevel, event: xdg_toplevel::Event) {
    match event {
        xdg_toplevel::Event::Configure {
            width,
            height,
            states,
        } => host.window.asked(width, height, &states),
        xdg_toplevel::Event::Close => host.closed = true,
        _ => {}
    }
}

fn on_wp_fractional_scale_v1(
    host: &mut Host,
    _: WpFractionalScaleV1,
    event: wp_fractional_scale_v1::Event,
) {
    host.window.scaling_mut().handle_event(event);
    host.reshaped = true;
}

fn on_wl_surface(host: &mut Host, surface: WlSurface, event: wl_surface::Event) {
    if &surface != host.window.surface() {
        return;
    }
    match event {
        wl_surface::Event::Enter { output } => host.window.entered(output),
        wl_surface::Event::Leave { output } => host.window.left(&output),
        wl_surface::Event::PreferredBufferScale { factor } => {
            host.window.scaling_mut().set_preferred_buffer_scale(factor);
        }
        _ => return,
    }
    host.reshaped = true;
}

fn on_wl_callback(host: &mut Host, _: WlCallback, event: wl_callback::Event) {
    if let wl_callback::Event::Done { .. } = event {
        host.frame_due = true;
    }
}
