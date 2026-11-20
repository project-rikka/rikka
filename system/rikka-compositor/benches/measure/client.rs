use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};

use rikka_test_display::Canvas;
use rikka_wayland_client::Queue;
use rikka_wayland_client::{
    ext_background_effect_manager_v1::ExtBackgroundEffectManagerV1,
    ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1,
};
use rikka_wayland_client::{
    wl_callback, wl_compositor, wl_keyboard, wl_pointer, wl_seat, wl_shm, wl_surface,
};
use rikka_wayland_client::{xdg_surface, xdg_toplevel, xdg_wm_base};
use rikka_wayland_client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

const PATIENCE: Duration = Duration::from_secs(20);

const IDLE_STEP: Duration = Duration::from_millis(10);

#[derive(Debug, Default)]
pub(crate) struct Recorded {
    serials: HashMap<u32, u32>,
    sizes: HashMap<u32, (i32, i32)>,
    layers: HashMap<u32, (u32, u32, u32)>,
    frames: HashMap<u32, u64>,
    pointed: Option<(u32, f64, f64)>,
    configured: HashMap<u32, Vec<Instant>>,
}

pub(crate) struct Painted {
    pub(crate) tag: u32,
    pub(crate) surface: wl_surface::WlSurface,
    pub(crate) canvas: Canvas,
}

pub(crate) struct Client {
    wayland: rikka_test_display::Client<Recorded>,
    compositor: wl_compositor::WlCompositor,
    shm: wl_shm::WlShm,
    toplevels: HashMap<u32, (xdg_surface::XdgSurface, xdg_toplevel::XdgToplevel)>,
    _pointer: wl_pointer::WlPointer,
    _keyboard: wl_keyboard::WlKeyboard,
}

impl Client {
    pub(crate) fn connect(socket: &Path) -> Self {
        let mut wayland =
            rikka_test_display::Client::connect_to(socket, listen).with_patience(PATIENCE);
        let compositor = wayland.global(6);
        let shm = wayland.global(1);
        let seat: wl_seat::WlSeat = wayland.global(7);
        let pointer = seat.get_pointer();
        let keyboard = seat.get_keyboard();
        wayland.roundtrip();
        Self {
            wayland,
            compositor,
            shm,
            toplevels: HashMap::new(),
            _pointer: pointer,
            _keyboard: keyboard,
        }
    }

    fn canvas(&self, width: i32, height: i32) -> Canvas {
        Canvas::new(&self.shm, width, height, wl_shm::Format::Argb8888)
    }

    pub(crate) fn window(&mut self, tag: u32, size: (i32, i32)) -> Painted {
        Painted {
            tag,
            canvas: self.canvas(size.0, size.1),
            surface: self.toplevel(tag),
        }
    }

    pub(crate) fn toplevel(&mut self, tag: u32) -> wl_surface::WlSurface {
        let wm_base: xdg_wm_base::XdgWmBase = self.wayland.global(6);
        let surface = self.compositor.create_surface().with_key(tag);
        let xdg = wm_base.get_xdg_surface(&surface).with_key(tag);
        let toplevel = xdg.get_toplevel().with_key(tag);
        surface.commit();
        self.wayland.until("toplevel configure", |recorded| {
            recorded.serials.contains_key(&tag)
        });
        xdg.ack_configure(self.wayland.received.serials[&tag]);
        self.toplevels.insert(tag, (xdg, toplevel));
        surface
    }

    pub(crate) fn maximize(&mut self, tag: u32, maximized: bool) {
        let (_, toplevel) = &self.toplevels[&tag];
        match maximized {
            true => toplevel.set_maximized(),
            false => toplevel.unset_maximized(),
        }
        self.wayland.connection.flush().unwrap();
    }

    pub(crate) fn destroy(&mut self, tag: u32, surface: &wl_surface::WlSurface) {
        if let Some((xdg, toplevel)) = self.toplevels.remove(&tag) {
            toplevel.destroy();
            xdg.destroy();
        }
        surface.destroy();
        self.wayland.connection.flush().unwrap();
        let recorded = &mut self.wayland.received;
        recorded.serials.remove(&tag);
        recorded.sizes.remove(&tag);
        recorded.configured.remove(&tag);
    }

    pub(crate) fn display(&self) -> rikka_wayland::Display {
        rikka_wayland::Display::from_connection(
            self.wayland.connection.clone(),
            self.wayland.display(),
        )
        .expect("shared client display")
    }

    pub(crate) fn configured(&self, tag: u32) -> &[Instant] {
        self.wayland
            .received
            .configured
            .get(&tag)
            .map_or(&[], Vec::as_slice)
    }

    pub(crate) fn ask_frame(&mut self, surface: &wl_surface::WlSurface, tag: u32) {
        surface.frame().set_key(tag);
    }

    pub(crate) fn layer(
        &mut self,
        tag: u32,
        layer: zwlr_layer_shell_v1::Layer,
        anchor: zwlr_layer_surface_v1::Anchor,
        size: (u32, u32),
        namespace: &str,
    ) -> Painted {
        let shell: zwlr_layer_shell_v1::ZwlrLayerShellV1 = self.wayland.global(4);
        let surface = self.compositor.create_surface().with_key(tag);
        let role = shell
            .get_layer_surface(&surface, None, layer, namespace)
            .with_key(tag);
        role.set_anchor(anchor);
        role.set_size(size.0, size.1);
        surface.commit();
        self.wayland.until("layer configure", |recorded| {
            recorded.layers.contains_key(&tag)
        });
        let (serial, width, height) = self.wayland.received.layers[&tag];
        role.ack_configure(serial);
        Painted {
            tag,
            canvas: self.canvas(width as i32, height as i32),
            surface,
        }
    }

    pub(crate) fn frost(
        &mut self,
        surface: &wl_surface::WlSurface,
        rects: &[(i32, i32, i32, i32)],
    ) -> ExtBackgroundEffectSurfaceV1 {
        let effects: ExtBackgroundEffectManagerV1 = self.wayland.global(1);
        let effect = effects.get_background_effect(surface);
        self.blur(&effect, rects);
        effect
    }

    pub(crate) fn blur(
        &self,
        effect: &ExtBackgroundEffectSurfaceV1,
        rects: &[(i32, i32, i32, i32)],
    ) {
        let region = self.compositor.create_region();
        for &(x, y, width, height) in rects {
            region.add(x, y, width, height);
        }
        effect.set_blur_region(Some(&region));
        region.destroy();
    }

    pub(crate) fn show(&mut self, painted: &Painted, damage: &[(i32, i32, i32, i32)]) {
        let surface = &painted.surface;
        surface.attach(Some(&painted.canvas.buffer), 0, 0);
        for &(x, y, width, height) in damage {
            surface.damage_buffer(x, y, width, height);
        }
        surface.frame().set_key(painted.tag);
        surface.commit();
        self.wayland.connection.flush().unwrap();
    }

    pub(crate) fn tick(&mut self, surface: &wl_surface::WlSurface, tag: u32) {
        surface.frame().set_key(tag);
        surface.commit();
        self.wayland.connection.flush().unwrap();
    }

    pub(crate) fn await_resize(&mut self, tag: u32, from: (i32, i32)) -> (i32, i32) {
        let resized = |recorded: &Recorded| {
            recorded
                .sizes
                .get(&tag)
                .is_some_and(|&size| size != from && size.0 > 0 && size.1 > 0)
        };
        self.wayland.until("resize", resized);
        self.wayland.received.sizes[&tag]
    }

    pub(crate) fn idle_for(&mut self, spell: Duration) {
        let until = Instant::now() + spell;
        while Instant::now() < until {
            std::thread::sleep(IDLE_STEP);
            self.roundtrip();
        }
    }

    pub(crate) fn size(&self, tag: u32) -> Option<(i32, i32)> {
        self.wayland.received.sizes.get(&tag).copied()
    }

    pub(crate) fn repaint(&self, painted: &mut Painted, width: i32, height: i32) {
        painted.canvas = self.canvas(width, height);
    }

    pub(crate) fn frames(&self, tag: u32) -> u64 {
        self.wayland.received.frames.get(&tag).copied().unwrap_or(0)
    }

    pub(crate) fn await_frame(&mut self, tag: u32, after: u64) {
        self.wayland.until("frame", |recorded| {
            recorded.frames.get(&tag).copied().unwrap_or(0) > after
        });
    }

    pub(crate) fn pointed(&mut self) -> Option<(u32, f64, f64)> {
        self.wayland.roundtrip();
        self.wayland.received.pointed
    }

    pub(crate) fn roundtrip(&mut self) {
        self.wayland.roundtrip();
    }
}

fn on_wl_callback(
    recorded: &mut Recorded,
    proxy: wl_callback::WlCallback,
    event: wl_callback::Event,
) {
    let Some(tag) = proxy.key::<u32>() else {
        return;
    };
    if let wl_callback::Event::Done { .. } = event {
        *recorded.frames.entry(tag).or_default() += 1;
    }
}

fn on_xdg_wm_base(_: &mut Recorded, wm_base: xdg_wm_base::XdgWmBase, event: xdg_wm_base::Event) {
    if let xdg_wm_base::Event::Ping { serial } = event {
        wm_base.pong(serial);
    }
}

fn on_xdg_surface(
    recorded: &mut Recorded,
    xdg: xdg_surface::XdgSurface,
    event: xdg_surface::Event,
) {
    let Some(tag) = xdg.key::<u32>() else {
        return;
    };
    if let xdg_surface::Event::Configure { serial } = event {
        recorded
            .configured
            .entry(tag)
            .or_default()
            .push(Instant::now());
        if recorded.serials.insert(tag, serial).is_some() {
            xdg.ack_configure(serial);
        }
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
    if let xdg_toplevel::Event::Configure { width, height, .. } = event {
        recorded.sizes.insert(tag, (width, height));
    }
}

fn on_zwlr_layer_surface_v1(
    recorded: &mut Recorded,
    proxy: zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    event: zwlr_layer_surface_v1::Event,
) {
    let Some(tag) = proxy.key::<u32>() else {
        return;
    };
    if let zwlr_layer_surface_v1::Event::Configure {
        serial,
        width,
        height,
    } = event
    {
        recorded.layers.insert(tag, (serial, width, height));
    }
}

fn on_wl_pointer(recorded: &mut Recorded, _: wl_pointer::WlPointer, event: wl_pointer::Event) {
    match event {
        wl_pointer::Event::Enter {
            surface_x,
            surface_y,
            surface,
            ..
        } => {
            let tag = surface.key::<u32>().unwrap_or(0);
            recorded.pointed = Some((tag, surface_x, surface_y));
        }
        wl_pointer::Event::Motion {
            surface_x,
            surface_y,
            ..
        } => {
            if let Some(pointed) = &mut recorded.pointed {
                pointed.1 = surface_x;
                pointed.2 = surface_y;
            }
        }
        wl_pointer::Event::Leave { .. } => recorded.pointed = None,
        _ => {}
    }
}

fn on_wl_surface(_: &mut Recorded, _: wl_surface::WlSurface, _: wl_surface::Event) {}

fn listen(queue: &mut Queue<Recorded>) {
    queue.on::<wl_callback::WlCallback>(on_wl_callback);
    queue.on::<xdg_wm_base::XdgWmBase>(on_xdg_wm_base);
    queue.on::<xdg_surface::XdgSurface>(on_xdg_surface);
    queue.on::<xdg_toplevel::XdgToplevel>(on_xdg_toplevel);
    queue.on::<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1>(on_zwlr_layer_surface_v1);
    queue.on::<wl_pointer::WlPointer>(on_wl_pointer);
    queue.on::<wl_surface::WlSurface>(on_wl_surface);
}
