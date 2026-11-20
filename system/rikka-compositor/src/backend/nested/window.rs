use rikka_types::PixelSize;
use rikka_wayland::{Configures, Display, FractionalScaleManager, Scale, Scaling};
use rikka_wayland_client::Route;
use rikka_wayland_client::wl_compositor::WlCompositor;
use rikka_wayland_client::wl_output::WlOutput;
use rikka_wayland_client::wl_surface::WlSurface;
use rikka_wayland_client::xdg_surface::XdgSurface;
use rikka_wayland_client::xdg_toplevel::{State, XdgToplevel};
use rikka_wayland_client::xdg_wm_base::XdgWmBase;
use rikka_wayland_client::zxdg_decoration_manager_v1::ZxdgDecorationManagerV1;
use rikka_wayland_client::zxdg_toplevel_decoration_v1::{Mode, ZxdgToplevelDecorationV1};

use super::screens::Screens;

const OPENING: (u32, u32) = (1280, 800);

const SCALING: u32 = 0;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Asked {
    size: Option<(u32, u32)>,
    resizing: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Shape {
    pub(super) logical: (u32, u32),
    pub(super) pixels: PixelSize,
    pub(super) scale: f64,
    pub(super) resizing: bool,
}

#[derive(Debug)]
pub(super) struct Window {
    surface: WlSurface,
    xdg: XdgSurface,
    toplevel: XdgToplevel,
    decoration: Option<ZxdgToplevelDecorationV1>,
    scaling: Scaling,
    configures: Configures<(u32, u32)>,
    asked: Asked,
    size: Option<(u32, u32)>,
    resizing: bool,
    entered: Vec<WlOutput>,
    placed: Option<(u32, u32)>,
}

impl Window {
    pub(super) fn open(
        display: &Display,
        route: &Route,
        title: &str,
        app_id: &str,
    ) -> rikka_wayland::Result<Self> {
        let compositor: WlCompositor = display.bind_on(route, 4..=6)?;
        let wm_base: XdgWmBase = display.bind_on(route, 1..=6)?;
        let decorations: Option<ZxdgDecorationManagerV1> = display.bind_optional_on(route, 1..=1);
        let fractional = FractionalScaleManager::bind(display, route);

        let surface = compositor.create_surface();
        let xdg = wm_base.get_xdg_surface(&surface);
        let toplevel = xdg.get_toplevel();
        toplevel.set_title(title);
        toplevel.set_app_id(app_id);
        let decoration = decorations.map(|manager| {
            let decoration = manager.get_toplevel_decoration(&toplevel);
            decoration.set_mode(Mode::ServerSide);
            decoration
        });
        let scaling = Scaling::new(fractional.as_ref(), &surface, SCALING);
        surface.commit();
        Ok(Self {
            surface,
            xdg,
            toplevel,
            decoration,
            scaling,
            configures: Configures::new(),
            asked: Asked::default(),
            size: None,
            resizing: false,
            entered: Vec::new(),
            placed: None,
        })
    }

    pub(super) fn surface(&self) -> &WlSurface {
        &self.surface
    }

    pub(super) fn is_configured(&self) -> bool {
        self.size.is_some()
    }

    pub(super) fn asked(&mut self, width: i32, height: i32, states: &[u8]) {
        let side = |length: i32| u32::try_from(length).ok().filter(|length| *length > 0);
        self.asked =
            Asked {
                size: side(width).zip(side(height)),
                resizing: states.as_chunks::<4>().0.iter().any(|state| {
                    State::try_from(u32::from_ne_bytes(*state)) == Ok(State::Resizing)
                }),
            };
    }

    pub(super) fn configured(&mut self, serial: u32) {
        let size = self.asked.size.or(self.size).unwrap_or(OPENING);
        self.configures.push(serial, size);
        self.size = Some(size);
        self.resizing = self.asked.resizing;
    }

    pub(super) fn scaling_mut(&mut self) -> &mut Scaling {
        &mut self.scaling
    }

    pub(super) fn entered(&mut self, output: WlOutput) {
        if !self.entered.contains(&output) {
            self.entered.push(output);
        }
    }

    pub(super) fn left(&mut self, output: &WlOutput) {
        self.entered.retain(|entered| entered != output);
    }

    pub(super) fn outputs(&self) -> &[WlOutput] {
        &self.entered
    }

    pub(super) fn shape(&self, screens: &Screens) -> Shape {
        let logical = self.size.unwrap_or(OPENING);
        let scale: Scale = self.scaling.scale(|| screens.scale_of(&self.entered));
        Shape {
            logical,
            pixels: scale.physical(PixelSize::new(logical.0, logical.1)),
            scale: scale.factor(),
            resizing: self.resizing,
        }
    }

    pub(super) fn frame(&mut self, drawn: (u32, u32)) {
        if let Some(serial) = self.configures.ack_for_commit(&drawn) {
            self.xdg.ack_configure(serial);
        }
        if self.placed.replace(drawn) != Some(drawn) {
            let side = |length: u32| i32::try_from(length).unwrap_or(i32::MAX);
            self.xdg
                .set_window_geometry(0, 0, side(drawn.0), side(drawn.1));
        }
        self.surface.frame();
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        if let Some(decoration) = self.decoration.take() {
            decoration.destroy();
        }
        self.toplevel.destroy();
        self.xdg.destroy();
        self.surface.destroy();
    }
}
