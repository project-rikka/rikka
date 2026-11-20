use rikka_seat::Grab;
use rikka_types::{Point, ResizeEdge, Size};
use rikka_wayland::Scaling;
use rikka_wayland_client::wl_surface::WlSurface;
use rikka_wayland_client::xdg_dialog_v1::XdgDialogV1;
use rikka_wayland_client::xdg_surface::XdgSurface;
use rikka_wayland_client::xdg_toplevel::{self, XdgToplevel};
use rikka_wayland_client::zxdg_exported_v2::ZxdgExportedV2;
use rikka_wayland_client::zxdg_imported_v2::ZxdgImportedV2;
use rikka_wayland_client::zxdg_toplevel_decoration_v1::{Mode, ZxdgToplevelDecorationV1};

use crate::tasks::WindowId;
use crate::wayland::globals::Globals;
use crate::wayland::scale::SurfaceSize;

#[derive(Clone, Copy, Debug)]
pub(crate) enum ParentToplevel<'a> {
    Ours(&'a XdgToplevel),
    Exported(&'a str),
}

pub(crate) struct ToplevelConfig<'a> {
    pub(crate) app_id: &'a str,
    pub(crate) title: &'a str,
    pub(crate) min_size: Option<Size>,
    pub(crate) fixed: Option<Size>,
    pub(crate) maximized: bool,
    pub(crate) server_decorations: bool,
    pub(crate) expected_scale: Option<f64>,
    pub(crate) parent: Option<ParentToplevel<'a>>,
    pub(crate) modal: bool,
}

pub(crate) struct Toplevel {
    surface: WlSurface,
    xdg: XdgSurface,
    toplevel: XdgToplevel,
    decoration: Option<ZxdgToplevelDecorationV1>,
    dialog: Option<XdgDialogV1>,
    imported: Option<ZxdgImportedV2>,
    exported: Option<ZxdgExportedV2>,
    scaling: Scaling,
    placed: Option<(u32, u32)>,
}

impl Toplevel {
    pub(crate) fn open(globals: &Globals, id: WindowId, config: &ToplevelConfig<'_>) -> Toplevel {
        let surface = globals.compositor.create_surface().with_key(id);
        let xdg = globals.wm_base.get_xdg_surface(&surface).with_key(id);
        let toplevel = xdg.get_toplevel().with_key(id);
        toplevel.set_app_id(config.app_id);
        toplevel.set_title(config.title);
        if let Some(min) = config.fixed.or(config.min_size) {
            toplevel.set_min_size(whole(min.width), whole(min.height));
        }
        if let Some(fixed) = config.fixed {
            toplevel.set_max_size(whole(fixed.width), whole(fixed.height));
        }
        if config.maximized {
            toplevel.set_maximized();
        }
        let decoration = globals.decorations.as_ref().map(|manager| {
            let decoration = manager.get_toplevel_decoration(&toplevel).with_key(id);
            decoration.set_mode(match config.server_decorations {
                true => Mode::ServerSide,
                false => Mode::ClientSide,
            });
            decoration
        });
        let imported = match config.parent {
            Some(ParentToplevel::Ours(parent)) => {
                toplevel.set_parent(Some(parent));
                None
            }
            Some(ParentToplevel::Exported(handle)) => import_parent(globals, id, handle, &surface),
            None => None,
        };
        let dialog = match (config.modal, globals.dialogs.as_ref()) {
            (true, Some(dialogs)) => {
                let dialog = dialogs.get_xdg_dialog(&toplevel);
                dialog.set_modal();
                Some(dialog)
            }
            (true, None) => {
                tracing::debug!("xdg-dialog unsupported: window opens without modality");
                None
            }
            (false, _) => None,
        };
        let exported = globals
            .exporter
            .as_ref()
            .map(|exporter| exporter.export_toplevel(&surface).with_key(id));
        let mut scaling = Scaling::new(globals.fractional_scale.as_ref(), &surface, id);
        if let Some(expected) = config.expected_scale {
            scaling.set_initial_factor(expected);
        }
        surface.commit();
        Toplevel {
            surface,
            xdg,
            toplevel,
            decoration,
            dialog,
            imported,
            exported,
            scaling,
            placed: None,
        }
    }

    pub(crate) fn surface(&self) -> &WlSurface {
        &self.surface
    }

    pub(crate) fn xdg_toplevel(&self) -> &XdgToplevel {
        &self.toplevel
    }

    pub(crate) fn scaling(&self) -> &Scaling {
        &self.scaling
    }

    pub(crate) fn scaling_mut(&mut self) -> &mut Scaling {
        &mut self.scaling
    }

    pub(crate) fn forget_parent(&mut self) {
        if let Some(imported) = self.imported.take() {
            imported.destroy();
        }
    }

    pub(crate) fn set_title(&self, title: &str) {
        self.toplevel.set_title(title);
    }

    pub(crate) fn set_maximized(&self, maximized: bool) {
        match maximized {
            true => self.toplevel.set_maximized(),
            false => self.toplevel.unset_maximized(),
        }
    }

    pub(crate) fn set_fullscreen(&self, fullscreen: bool) {
        match fullscreen {
            true => self.toplevel.set_fullscreen(None),
            false => self.toplevel.unset_fullscreen(),
        }
    }

    pub(crate) fn minimize(&self) {
        self.toplevel.set_minimized();
    }

    pub(crate) fn start_move(&self, grab: &Grab) {
        self.toplevel.r#move(&grab.seat, grab.serial);
    }

    pub(crate) fn start_resize(&self, grab: &Grab, edge: ResizeEdge) {
        self.toplevel
            .resize(&grab.seat, grab.serial, resize_edge(edge));
    }

    pub(crate) fn show_menu(&self, grab: &Grab, at: Point) {
        self.toplevel.show_window_menu(
            &grab.seat,
            grab.serial,
            at.x.round() as i32,
            at.y.round() as i32,
        );
    }

    pub(crate) fn frame(&self, id: WindowId) {
        self.surface.frame().set_key(id);
    }

    pub(crate) fn ack(&self, serial: u32) {
        self.xdg.ack_configure(serial);
    }

    pub(crate) fn commit(&self) {
        self.surface.commit();
    }

    pub(crate) fn place(&mut self, drawn: SurfaceSize) {
        if self.placed.replace(drawn.logical) == Some(drawn.logical) {
            return;
        }
        let (width, height) = drawn.logical;
        self.xdg
            .set_window_geometry(0, 0, signed(width), signed(height));
    }
}

impl Drop for Toplevel {
    fn drop(&mut self) {
        drop(std::mem::take(&mut self.scaling));
        if let Some(decoration) = self.decoration.take() {
            decoration.destroy();
        }
        if let Some(dialog) = self.dialog.take() {
            dialog.destroy();
        }
        if let Some(exported) = self.exported.take() {
            exported.destroy();
        }
        self.forget_parent();
        self.toplevel.destroy();
        self.xdg.destroy();
        self.surface.destroy();
    }
}

impl std::fmt::Debug for Toplevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Toplevel")
            .field("surface", &self.surface.id())
            .finish_non_exhaustive()
    }
}

fn import_parent(
    globals: &Globals,
    id: WindowId,
    handle: &str,
    surface: &WlSurface,
) -> Option<ZxdgImportedV2> {
    let Some(importer) = globals.importer.as_ref() else {
        tracing::warn!("xdg-foreign unsupported: window opens without its parent");
        return None;
    };
    let imported = importer.import_toplevel(handle).with_key(id);
    imported.set_parent_of(surface);
    Some(imported)
}

fn whole(logical: f32) -> i32 {
    logical.round().max(0.0) as i32
}

fn signed(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

fn resize_edge(edge: ResizeEdge) -> xdg_toplevel::ResizeEdge {
    match edge {
        ResizeEdge::Top => xdg_toplevel::ResizeEdge::Top,
        ResizeEdge::Bottom => xdg_toplevel::ResizeEdge::Bottom,
        ResizeEdge::Left => xdg_toplevel::ResizeEdge::Left,
        ResizeEdge::Right => xdg_toplevel::ResizeEdge::Right,
        ResizeEdge::TopLeft => xdg_toplevel::ResizeEdge::TopLeft,
        ResizeEdge::TopRight => xdg_toplevel::ResizeEdge::TopRight,
        ResizeEdge::BottomLeft => xdg_toplevel::ResizeEdge::BottomLeft,
        ResizeEdge::BottomRight => xdg_toplevel::ResizeEdge::BottomRight,
    }
}
