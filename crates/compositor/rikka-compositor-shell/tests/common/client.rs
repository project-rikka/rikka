use std::ops::{Deref, DerefMut};
use std::os::unix::net::UnixStream;

use rikka_test_display::Client;
use rikka_wayland_client::{
    Queue, ext_foreign_toplevel_handle_v1, ext_foreign_toplevel_list_v1,
    ext_session_lock_surface_v1, ext_session_lock_v1, wl_compositor, wl_surface,
    xdg_activation_token_v1, xdg_popup, xdg_positioner, xdg_surface, xdg_toplevel,
    xdg_toplevel_icon_manager_v1, xdg_wm_base, zwlr_layer_surface_v1, zxdg_exported_v2,
    zxdg_imported_v2, zxdg_toplevel_decoration_v1,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ToplevelConfigured {
    pub(crate) tag: u32,
    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) states: Vec<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ForeignHandle {
    pub(crate) id: u32,
    pub(crate) identifier: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) app_id: Option<String>,
    pub(crate) done: u32,
    pub(crate) closed: bool,
}

#[derive(Debug, Default)]
pub(crate) struct Received {
    pub(crate) pings: Vec<u32>,
    pub(crate) surface_configures: Vec<(u32, u32)>,
    pub(crate) toplevel_configures: Vec<ToplevelConfigured>,
    pub(crate) bounds: Vec<(i32, i32)>,
    pub(crate) capabilities: Vec<Vec<u32>>,
    pub(crate) closed: Vec<u32>,
    pub(crate) popup_configures: Vec<(u32, i32, i32, i32, i32)>,
    pub(crate) popups_done: Vec<u32>,
    pub(crate) repositioned: Vec<(u32, u32)>,
    pub(crate) decorations: Vec<u32>,
    pub(crate) tokens: Vec<String>,
    pub(crate) layer_configures: Vec<(u32, u32, u32, u32)>,
    pub(crate) layers_closed: Vec<u32>,
    pub(crate) locked: Vec<u32>,
    pub(crate) finished: Vec<u32>,
    pub(crate) lock_configures: Vec<(u32, u32, u32)>,
    pub(crate) foreign: Vec<ForeignHandle>,
    pub(crate) exported: Vec<String>,
    pub(crate) imports_destroyed: Vec<u32>,
    pub(crate) icon_sizes: Vec<i32>,
    pub(crate) icon_done: bool,
}

impl Received {
    pub(crate) fn last_configure(&self, tag: u32) -> u32 {
        self.surface_configures
            .iter()
            .rev()
            .find(|(received, _)| *received == tag)
            .map(|(_, serial)| *serial)
            .expect("the surface was configured")
    }

    fn foreign_handle(
        &mut self,
        handle: &ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
    ) -> &mut ForeignHandle {
        let id = handle.id();
        if let Some(index) = self.foreign.iter().position(|known| known.id == id) {
            return &mut self.foreign[index];
        }
        self.foreign.push(ForeignHandle {
            id,
            ..ForeignHandle::default()
        });
        self.foreign.last_mut().unwrap()
    }
}

fn on_xdg_wm_base(received: &mut Received, _: xdg_wm_base::XdgWmBase, event: xdg_wm_base::Event) {
    if let xdg_wm_base::Event::Ping { serial } = event {
        received.pings.push(serial);
    }
}

fn on_xdg_surface(
    received: &mut Received,
    proxy: xdg_surface::XdgSurface,
    event: xdg_surface::Event,
) {
    let tag = &proxy.key::<u32>().expect("the object is tagged");
    if let xdg_surface::Event::Configure { serial } = event {
        received.surface_configures.push((*tag, serial));
    }
}

fn on_xdg_toplevel(
    received: &mut Received,
    proxy: xdg_toplevel::XdgToplevel,
    event: xdg_toplevel::Event,
) {
    let tag = &proxy.key::<u32>().expect("the object is tagged");
    match event {
        xdg_toplevel::Event::Configure {
            width,
            height,
            states,
        } => received.toplevel_configures.push(ToplevelConfigured {
            tag: *tag,
            width,
            height,
            states: words(&states),
        }),
        xdg_toplevel::Event::ConfigureBounds { width, height } => {
            received.bounds.push((width, height));
        }
        xdg_toplevel::Event::WmCapabilities { capabilities } => {
            received.capabilities.push(words(&capabilities));
        }
        xdg_toplevel::Event::Close => received.closed.push(*tag),
        _ => {}
    }
}

fn on_xdg_popup(received: &mut Received, proxy: xdg_popup::XdgPopup, event: xdg_popup::Event) {
    let tag = &proxy.key::<u32>().expect("the object is tagged");
    match event {
        xdg_popup::Event::Configure {
            x,
            y,
            width,
            height,
        } => received.popup_configures.push((*tag, x, y, width, height)),
        xdg_popup::Event::PopupDone => received.popups_done.push(*tag),
        xdg_popup::Event::Repositioned { token } => received.repositioned.push((*tag, token)),
        _ => {}
    }
}

fn on_zxdg_toplevel_decoration_v1(
    received: &mut Received,
    _: zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1,
    event: zxdg_toplevel_decoration_v1::Event,
) {
    if let zxdg_toplevel_decoration_v1::Event::Configure { mode } = event {
        received
            .decorations
            .push(rikka_wayland_client::raw_value(mode));
    }
}

fn on_xdg_activation_token_v1(
    received: &mut Received,
    _: xdg_activation_token_v1::XdgActivationTokenV1,
    event: xdg_activation_token_v1::Event,
) {
    if let xdg_activation_token_v1::Event::Done { token } = event {
        received.tokens.push(token);
    }
}

fn on_zwlr_layer_surface_v1(
    received: &mut Received,
    proxy: zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    event: zwlr_layer_surface_v1::Event,
) {
    let tag = &proxy.key::<u32>().expect("the object is tagged");
    match event {
        zwlr_layer_surface_v1::Event::Configure {
            serial,
            width,
            height,
        } => received
            .layer_configures
            .push((*tag, serial, width, height)),
        zwlr_layer_surface_v1::Event::Closed => received.layers_closed.push(*tag),
        _ => {}
    }
}

fn on_ext_session_lock_v1(
    received: &mut Received,
    proxy: ext_session_lock_v1::ExtSessionLockV1,
    event: ext_session_lock_v1::Event,
) {
    let tag = &proxy.key::<u32>().expect("the object is tagged");
    match event {
        ext_session_lock_v1::Event::Locked => received.locked.push(*tag),
        ext_session_lock_v1::Event::Finished => received.finished.push(*tag),
        _ => {}
    }
}

fn on_ext_session_lock_surface_v1(
    received: &mut Received,
    _: ext_session_lock_surface_v1::ExtSessionLockSurfaceV1,
    event: ext_session_lock_surface_v1::Event,
) {
    if let ext_session_lock_surface_v1::Event::Configure {
        serial,
        width,
        height,
    } = event
    {
        received.lock_configures.push((serial, width, height));
    }
}

fn on_ext_foreign_toplevel_list_v1(
    received: &mut Received,
    _: ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1,
    event: ext_foreign_toplevel_list_v1::Event,
) {
    if let ext_foreign_toplevel_list_v1::Event::Toplevel { toplevel } = event {
        received.foreign_handle(&toplevel);
    }
}

fn on_ext_foreign_toplevel_handle_v1(
    received: &mut Received,
    handle: ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
    event: ext_foreign_toplevel_handle_v1::Event,
) {
    let record = received.foreign_handle(&handle);
    match event {
        ext_foreign_toplevel_handle_v1::Event::Identifier { identifier } => {
            record.identifier = Some(identifier);
        }
        ext_foreign_toplevel_handle_v1::Event::Title { title } => record.title = Some(title),
        ext_foreign_toplevel_handle_v1::Event::AppId { app_id } => {
            record.app_id = Some(app_id);
        }
        ext_foreign_toplevel_handle_v1::Event::Done => record.done += 1,
        ext_foreign_toplevel_handle_v1::Event::Closed => record.closed = true,
        _ => {}
    }
}

fn on_zxdg_exported_v2(
    received: &mut Received,
    _: zxdg_exported_v2::ZxdgExportedV2,
    event: zxdg_exported_v2::Event,
) {
    if let zxdg_exported_v2::Event::Handle { handle } = event {
        received.exported.push(handle);
    }
}

fn on_zxdg_imported_v2(
    received: &mut Received,
    proxy: zxdg_imported_v2::ZxdgImportedV2,
    event: zxdg_imported_v2::Event,
) {
    let tag = &proxy.key::<u32>().expect("the object is tagged");
    if let zxdg_imported_v2::Event::Destroyed = event {
        received.imports_destroyed.push(*tag);
    }
}

fn on_xdg_toplevel_icon_manager_v1(
    received: &mut Received,
    _: xdg_toplevel_icon_manager_v1::XdgToplevelIconManagerV1,
    event: xdg_toplevel_icon_manager_v1::Event,
) {
    match event {
        xdg_toplevel_icon_manager_v1::Event::IconSize { size } => received.icon_sizes.push(size),
        xdg_toplevel_icon_manager_v1::Event::Done => received.icon_done = true,
        _ => {}
    }
}

fn words(bytes: &[u8]) -> Vec<u32> {
    let (words, _) = bytes.as_chunks::<4>();
    words.iter().map(|word| u32::from_ne_bytes(*word)).collect()
}

pub(crate) struct Toplevel {
    pub(crate) surface: wl_surface::WlSurface,
    pub(crate) xdg_surface: xdg_surface::XdgSurface,
    pub(crate) toplevel: xdg_toplevel::XdgToplevel,
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
    queue.on::<xdg_wm_base::XdgWmBase>(on_xdg_wm_base);
    queue.on::<xdg_surface::XdgSurface>(on_xdg_surface);
    queue.on::<xdg_toplevel::XdgToplevel>(on_xdg_toplevel);
    queue.on::<xdg_popup::XdgPopup>(on_xdg_popup);
    queue.on::<zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1>(
        on_zxdg_toplevel_decoration_v1,
    );
    queue.on::<xdg_activation_token_v1::XdgActivationTokenV1>(on_xdg_activation_token_v1);
    queue.on::<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1>(on_zwlr_layer_surface_v1);
    queue.on::<ext_session_lock_v1::ExtSessionLockV1>(on_ext_session_lock_v1);
    queue
        .on::<ext_session_lock_surface_v1::ExtSessionLockSurfaceV1>(on_ext_session_lock_surface_v1);
    queue.on::<ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1>(
        on_ext_foreign_toplevel_list_v1,
    );
    queue.on::<ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1>(
        on_ext_foreign_toplevel_handle_v1,
    );
    queue.on::<zxdg_exported_v2::ZxdgExportedV2>(on_zxdg_exported_v2);
    queue.on::<zxdg_imported_v2::ZxdgImportedV2>(on_zxdg_imported_v2);
    queue.on::<xdg_toplevel_icon_manager_v1::XdgToplevelIconManagerV1>(
        on_xdg_toplevel_icon_manager_v1,
    );
}

impl Wayland {
    pub(crate) fn new(stream: UnixStream) -> Self {
        Self(Client::connect(stream, listen))
    }

    pub(crate) fn surface(&self) -> wl_surface::WlSurface {
        let compositor: wl_compositor::WlCompositor = self.global(6);
        compositor.create_surface()
    }

    pub(crate) fn wm_base(&self) -> xdg_wm_base::XdgWmBase {
        self.global(7)
    }

    pub(crate) fn toplevel(&self, wm_base: &xdg_wm_base::XdgWmBase, tag: u32) -> Toplevel {
        let surface = self.surface();
        let xdg_surface = wm_base.get_xdg_surface(&surface).with_key(tag);
        let toplevel = xdg_surface.get_toplevel().with_key(tag);
        Toplevel {
            surface,
            xdg_surface,
            toplevel,
        }
    }

    pub(crate) fn positioner(
        &self,
        wm_base: &xdg_wm_base::XdgWmBase,
        size: (i32, i32),
        anchor_rect: (i32, i32, i32, i32),
    ) -> xdg_positioner::XdgPositioner {
        let positioner = wm_base.create_positioner();
        positioner.set_size(size.0, size.1);
        positioner.set_anchor_rect(anchor_rect.0, anchor_rect.1, anchor_rect.2, anchor_rect.3);
        positioner
    }
}
