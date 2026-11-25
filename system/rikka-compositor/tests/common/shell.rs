use std::os::fd::OwnedFd;

use rikka_wayland_client::xdg_wm_dialog_v1;
use rikka_wayland_client::{
    ext_session_lock_manager_v1, ext_session_lock_surface_v1, ext_session_lock_v1,
};
use rikka_wayland_client::{wl_keyboard, wl_pointer, wl_seat, wl_surface};
use rikka_wayland_client::{xdg_popup, xdg_positioner, xdg_surface, xdg_toplevel, xdg_wm_base};
use rikka_wayland_client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};
use rikka_wayland_client::{zxdg_decoration_manager_v1, zxdg_toplevel_decoration_v1};

use rikka_session::surfaces;
use rikka_test_display::Canvas;

use super::client::{Client, Recorded};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Configured {
    pub(crate) tag: u32,
    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) states: Vec<u32>,
}

#[derive(Debug, Default)]
pub(crate) struct ShellRecord {
    pub(crate) configured: Vec<Configured>,
    pub(crate) serials: Vec<(u32, u32)>,
    pub(crate) closed: Vec<u32>,
    pub(crate) popup_placed: Vec<(u32, i32, i32, i32, i32)>,
    pub(crate) popup_done: Vec<u32>,
    pub(crate) layer_configured: Vec<(u32, u32, u32, u32)>,
    pub(crate) locked: bool,
    pub(crate) lock_configured: Vec<(u32, u32, u32)>,
    pub(crate) pointer_on: Option<u32>,
    pub(crate) pointer_entered: Option<u32>,
    pub(crate) pointer_at: Option<(f64, f64)>,
    pub(crate) scrolled: Vec<Scrolled>,
    pub(crate) keyboard_on: Option<u32>,
    pub(crate) pressed: Vec<u32>,
    pub(crate) keys: Vec<u32>,
    pub(crate) strokes: Vec<(u32, Mods)>,
    pub(crate) lifted: Vec<u32>,
    pub(crate) mods: Mods,
    pub(crate) keymaps: Vec<(OwnedFd, u32)>,
    pub(crate) decorations: Vec<(u32, zxdg_toplevel_decoration_v1::Mode)>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Mods {
    pub(crate) depressed: u32,
    pub(crate) latched: u32,
    pub(crate) locked: u32,
    pub(crate) group: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Scrolled {
    Source(wl_pointer::AxisSource),
    Value(wl_pointer::Axis, f64),
    Value120(wl_pointer::Axis, i32),
    Frame,
}

impl ShellRecord {
    pub(crate) fn last_configure(&self, tag: u32) -> Option<&Configured> {
        self.configured
            .iter()
            .rev()
            .find(|recorded| recorded.tag == tag)
    }

    pub(crate) fn serial(&self, tag: u32) -> Option<u32> {
        self.serials
            .iter()
            .rev()
            .find(|(recorded_tag, _)| *recorded_tag == tag)
            .map(|(_, serial)| *serial)
    }
}

pub(crate) struct Toplevel {
    pub(crate) tag: u32,
    pub(crate) surface: wl_surface::WlSurface,
    pub(crate) xdg: xdg_surface::XdgSurface,
    pub(crate) toplevel: xdg_toplevel::XdgToplevel,
}

pub(crate) struct Popup {
    pub(crate) surface: wl_surface::WlSurface,
    pub(crate) xdg: xdg_surface::XdgSurface,
    pub(crate) popup: xdg_popup::XdgPopup,
}

pub(crate) struct Devices {
    pub(crate) seat: wl_seat::WlSeat,
    pub(crate) pointer: wl_pointer::WlPointer,
}

impl Client {
    fn wm_base(&self) -> xdg_wm_base::XdgWmBase {
        self.global(6)
    }

    pub(crate) fn toplevel(&mut self, tag: u32) -> Toplevel {
        let wm_base = self.wm_base();
        let surface = self.surface();
        let xdg = wm_base.get_xdg_surface(&surface).with_key(tag);
        let toplevel = xdg.get_toplevel().with_key(tag);
        surface.commit();
        let window = Toplevel {
            tag,
            surface,
            xdg,
            toplevel,
        };
        self.until("first configure", |recorded| {
            recorded.shell.serial(tag).is_some()
        });
        window
    }

    pub(crate) fn decorated_toplevel(
        &mut self,
        tag: u32,
        mode: zxdg_toplevel_decoration_v1::Mode,
    ) -> Toplevel {
        let wm_base = self.wm_base();
        let decorations: zxdg_decoration_manager_v1::ZxdgDecorationManagerV1 = self.global(1);
        let surface = self.surface();
        let xdg = wm_base.get_xdg_surface(&surface).with_key(tag);
        let toplevel = xdg.get_toplevel().with_key(tag);
        decorations
            .get_toplevel_decoration(&toplevel)
            .with_key(tag)
            .set_mode(mode);
        surface.commit();
        let window = Toplevel {
            tag,
            surface,
            xdg,
            toplevel,
        };
        self.until("first decorated configure", |recorded| {
            recorded.shell.serial(tag).is_some()
        });
        window
    }

    pub(crate) fn dialog(&mut self, tag: u32, parent: &Toplevel, modal: bool) -> Toplevel {
        let wm_base = self.wm_base();
        let dialogs: xdg_wm_dialog_v1::XdgWmDialogV1 = self.global(1);
        let surface = self.surface();
        let xdg = wm_base.get_xdg_surface(&surface).with_key(tag);
        let toplevel = xdg.get_toplevel().with_key(tag);
        toplevel.set_parent(Some(&parent.toplevel));
        let dialog = dialogs.get_xdg_dialog(&toplevel);
        if modal {
            dialog.set_modal();
        }
        surface.commit();
        let window = Toplevel {
            tag,
            surface,
            xdg,
            toplevel,
        };
        self.until("first dialog configure", |recorded| {
            recorded.shell.serial(tag).is_some()
        });
        window
    }

    pub(crate) fn ack(&mut self, xdg: &xdg_surface::XdgSurface, tag: u32) {
        let serial = self.received.shell.serial(tag).expect("pending configure");
        xdg.ack_configure(serial);
    }

    pub(crate) fn show(&mut self, surface: &wl_surface::WlSurface, pixels: &Canvas) {
        surface.attach(Some(&pixels.buffer), 0, 0);
        surface.damage_buffer(0, 0, pixels.width, pixels.height);
        surface.commit();
        self.roundtrip();
    }

    pub(crate) fn popup(
        &mut self,
        parent: &Toplevel,
        tag: u32,
        at: (i32, i32),
        size: (i32, i32),
    ) -> Popup {
        let wm_base = self.wm_base();
        let positioner = wm_base.create_positioner();
        positioner.set_size(size.0, size.1);
        positioner.set_anchor_rect(at.0, at.1, 1, 1);
        positioner.set_anchor(xdg_positioner::Anchor::TopLeft);
        positioner.set_gravity(xdg_positioner::Gravity::BottomRight);
        let surface = self.surface();
        let xdg = wm_base.get_xdg_surface(&surface).with_key(tag);
        let popup = xdg.get_popup(Some(&parent.xdg), &positioner).with_key(tag);
        Popup {
            surface,
            xdg,
            popup,
        }
    }

    pub(crate) fn devices(&mut self) -> Devices {
        let seat: wl_seat::WlSeat = self.global(9);
        seat.get_keyboard();
        let devices = Devices {
            pointer: seat.get_pointer(),
            seat,
        };
        self.roundtrip();
        devices
    }

    pub(crate) fn layer(
        &mut self,
        tag: u32,
        anchor: zwlr_layer_surface_v1::Anchor,
        size: (u32, u32),
        exclusive: i32,
    ) -> (
        wl_surface::WlSurface,
        zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    ) {
        self.layer_on(
            zwlr_layer_shell_v1::Layer::Top,
            tag,
            anchor,
            size,
            exclusive,
        )
    }

    pub(crate) fn layer_on(
        &mut self,
        level: zwlr_layer_shell_v1::Layer,
        tag: u32,
        anchor: zwlr_layer_surface_v1::Anchor,
        size: (u32, u32),
        exclusive: i32,
    ) -> (
        wl_surface::WlSurface,
        zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    ) {
        self.named_layer(level, "test", tag, anchor, size, exclusive)
    }

    pub(crate) fn scrim(
        &mut self,
        tag: u32,
    ) -> (
        wl_surface::WlSurface,
        zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    ) {
        self.named_layer(
            zwlr_layer_shell_v1::Layer::Overlay,
            surfaces::SCRIM,
            tag,
            zwlr_layer_surface_v1::Anchor::all(),
            (0, 0),
            -1,
        )
    }

    fn named_layer(
        &mut self,
        level: zwlr_layer_shell_v1::Layer,
        namespace: &str,
        tag: u32,
        anchor: zwlr_layer_surface_v1::Anchor,
        size: (u32, u32),
        exclusive: i32,
    ) -> (
        wl_surface::WlSurface,
        zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    ) {
        let shell: zwlr_layer_shell_v1::ZwlrLayerShellV1 = self.global(4);
        let surface = self.surface();
        let layer = shell
            .get_layer_surface(&surface, None, level, namespace)
            .with_key(tag);
        layer.set_anchor(anchor);
        layer.set_size(size.0, size.1);
        layer.set_exclusive_zone(exclusive);
        surface.commit();
        self.until("first layer configure", |recorded| {
            recorded
                .shell
                .layer_configured
                .iter()
                .any(|(recorded_tag, ..)| *recorded_tag == tag)
        });
        (surface, layer)
    }

    pub(crate) fn lock(&mut self) -> ext_session_lock_v1::ExtSessionLockV1 {
        let manager: ext_session_lock_manager_v1::ExtSessionLockManagerV1 = self.global(1);
        manager.lock()
    }
}

fn on_xdg_wm_base(_: &mut Recorded, wm_base: xdg_wm_base::XdgWmBase, event: xdg_wm_base::Event) {
    if let xdg_wm_base::Event::Ping { serial } = event {
        wm_base.pong(serial);
    }
}

fn on_xdg_surface(
    recorded: &mut Recorded,
    proxy: xdg_surface::XdgSurface,
    event: xdg_surface::Event,
) {
    let Some(tag) = proxy.key::<u32>() else {
        return;
    };
    if let xdg_surface::Event::Configure { serial } = event {
        recorded.shell.serials.push((tag, serial));
    }
}

fn on_xdg_toplevel(
    recorded: &mut Recorded,
    proxy: xdg_toplevel::XdgToplevel,
    event: xdg_toplevel::Event,
) {
    let Some(tag) = proxy.key::<u32>() else {
        return;
    };
    match event {
        xdg_toplevel::Event::Configure {
            width,
            height,
            states,
        } => recorded.shell.configured.push(Configured {
            tag,
            width,
            height,
            states: states
                .as_chunks::<4>()
                .0
                .iter()
                .map(|state| u32::from_ne_bytes(*state))
                .collect(),
        }),
        xdg_toplevel::Event::Close => recorded.shell.closed.push(tag),
        _ => {}
    }
}

fn on_zxdg_toplevel_decoration_v1(
    recorded: &mut Recorded,
    proxy: zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1,
    event: zxdg_toplevel_decoration_v1::Event,
) {
    let Some(tag) = proxy.key::<u32>() else {
        return;
    };
    let zxdg_toplevel_decoration_v1::Event::Configure { mode: Ok(mode) } = event else {
        return;
    };
    recorded.shell.decorations.push((tag, mode));
}

fn on_xdg_popup(recorded: &mut Recorded, proxy: xdg_popup::XdgPopup, event: xdg_popup::Event) {
    let Some(tag) = proxy.key::<u32>() else {
        return;
    };
    match event {
        xdg_popup::Event::Configure {
            x,
            y,
            width,
            height,
        } => recorded.shell.popup_placed.push((tag, x, y, width, height)),
        xdg_popup::Event::PopupDone => recorded.shell.popup_done.push(tag),
        _ => {}
    }
}

fn on_zwlr_layer_surface_v1(
    recorded: &mut Recorded,
    layer: zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    event: zwlr_layer_surface_v1::Event,
) {
    let Some(tag) = layer.key::<u32>() else {
        return;
    };
    if let zwlr_layer_surface_v1::Event::Configure {
        serial,
        width,
        height,
    } = event
    {
        layer.ack_configure(serial);
        recorded
            .shell
            .layer_configured
            .push((tag, serial, width, height));
    }
}

fn on_ext_session_lock_v1(
    recorded: &mut Recorded,
    _: ext_session_lock_v1::ExtSessionLockV1,
    event: ext_session_lock_v1::Event,
) {
    if let ext_session_lock_v1::Event::Locked = event {
        recorded.shell.locked = true;
    }
}

fn on_ext_session_lock_surface_v1(
    recorded: &mut Recorded,
    surface: ext_session_lock_surface_v1::ExtSessionLockSurfaceV1,
    event: ext_session_lock_surface_v1::Event,
) {
    if let ext_session_lock_surface_v1::Event::Configure {
        serial,
        width,
        height,
    } = event
    {
        surface.ack_configure(serial);
        recorded.shell.lock_configured.push((serial, width, height));
    }
}

fn on_wl_pointer(
    recorded: &mut Recorded,
    pointer: wl_pointer::WlPointer,
    event: wl_pointer::Event,
) {
    if pointer.key::<u32>().is_some() {
        return super::input::tagged_pointer(recorded, event);
    }
    let shell = &mut recorded.shell;
    match event {
        wl_pointer::Event::Enter {
            serial,
            surface,
            surface_x,
            surface_y,
        } => {
            shell.pointer_on = Some(surface.id());
            shell.pointer_entered = Some(serial);
            shell.pointer_at = Some((surface_x, surface_y));
        }
        wl_pointer::Event::Leave { .. } => shell.pointer_on = None,
        wl_pointer::Event::Motion {
            surface_x,
            surface_y,
            ..
        } => shell.pointer_at = Some((surface_x, surface_y)),
        wl_pointer::Event::Button {
            serial,
            state: Ok(wl_pointer::ButtonState::Pressed),
            ..
        } => shell.pressed.push(serial),
        wl_pointer::Event::AxisSource {
            axis_source: Ok(source),
        } => shell.scrolled.push(Scrolled::Source(source)),
        wl_pointer::Event::Axis {
            axis: Ok(axis),
            value,
            ..
        } => shell.scrolled.push(Scrolled::Value(axis, value)),
        wl_pointer::Event::AxisValue120 {
            axis: Ok(axis),
            value120,
        } => shell.scrolled.push(Scrolled::Value120(axis, value120)),
        wl_pointer::Event::Frame
            if shell
                .scrolled
                .last()
                .is_some_and(|last| *last != Scrolled::Frame) =>
        {
            shell.scrolled.push(Scrolled::Frame);
        }
        _ => {}
    }
}

fn on_wl_keyboard(recorded: &mut Recorded, _: wl_keyboard::WlKeyboard, event: wl_keyboard::Event) {
    let shell = &mut recorded.shell;
    match event {
        wl_keyboard::Event::Keymap { fd, size, .. } => shell.keymaps.push((fd, size)),
        wl_keyboard::Event::Enter { surface, .. } => {
            shell.keyboard_on = Some(surface.id());
        }
        wl_keyboard::Event::Leave { .. } => shell.keyboard_on = None,
        wl_keyboard::Event::Key {
            key,
            state: Ok(wl_keyboard::KeyState::Pressed),
            ..
        } => {
            shell.keys.push(key);
            shell.strokes.push((key, shell.mods));
        }
        wl_keyboard::Event::Key {
            key,
            state: Ok(wl_keyboard::KeyState::Released),
            ..
        } => shell.lifted.push(key),
        wl_keyboard::Event::Modifiers {
            mods_depressed,
            mods_latched,
            mods_locked,
            group,
            ..
        } => {
            shell.mods = Mods {
                depressed: mods_depressed,
                latched: mods_latched,
                locked: mods_locked,
                group,
            };
        }
        _ => {}
    }
}

pub(crate) fn listen(queue: &mut rikka_wayland_client::Queue<Recorded>) {
    queue.on::<xdg_wm_base::XdgWmBase>(on_xdg_wm_base);
    queue.on::<xdg_surface::XdgSurface>(on_xdg_surface);
    queue.on::<xdg_toplevel::XdgToplevel>(on_xdg_toplevel);
    queue.on::<xdg_popup::XdgPopup>(on_xdg_popup);
    queue.on::<zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1>(
        on_zxdg_toplevel_decoration_v1,
    );
    queue.on::<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1>(on_zwlr_layer_surface_v1);
    queue.on::<ext_session_lock_v1::ExtSessionLockV1>(on_ext_session_lock_v1);
    queue
        .on::<ext_session_lock_surface_v1::ExtSessionLockSurfaceV1>(on_ext_session_lock_surface_v1);
    queue.on::<wl_pointer::WlPointer>(on_wl_pointer);
    queue.on::<wl_keyboard::WlKeyboard>(on_wl_keyboard);
}
