use rikka_seat::Seats;
use rikka_wayland::EventLoop;
use rikka_wayland_client::ObjectId;
use rikka_wayland_client::ext_idle_notification_v1::{self, ExtIdleNotificationV1};
use rikka_wayland_client::ext_session_lock_surface_v1::{self, ExtSessionLockSurfaceV1};
use rikka_wayland_client::ext_session_lock_v1::{self, ExtSessionLockV1};
use rikka_wayland_client::wl_callback::{self, WlCallback};
use rikka_wayland_client::wl_output::{self, WlOutput};
use rikka_wayland_client::wl_registry::{self, WlRegistry};
use rikka_wayland_client::wl_surface::{self, WlSurface};
use rikka_wayland_client::zwlr_layer_surface_v1::{self, ZwlrLayerSurfaceV1};
use rikka_wayland_client::zxdg_output_v1::{self, ZxdgOutputV1};

use super::state::ShellState;
use super::surfaces::{UnmapSync, Unmapping};

pub(super) fn listen(queue: &mut EventLoop<ShellState>) {
    Seats::listen(queue.queue_mut());
    queue.on::<WlRegistry>(on_wl_registry);
    queue.on::<WlOutput>(on_wl_output);
    queue.on::<ZxdgOutputV1>(on_zxdg_output_v1);
    queue.on::<WlSurface>(on_wl_surface);
    queue.on::<WlCallback>(on_wl_callback);
    queue.on::<ZwlrLayerSurfaceV1>(on_zwlr_layer_surface_v1);
    queue.on::<ExtSessionLockV1>(on_ext_session_lock_v1);
    queue.on::<ExtSessionLockSurfaceV1>(on_ext_session_lock_surface_v1);
    queue.on::<ExtIdleNotificationV1>(on_ext_idle_notification_v1);
}

fn on_wl_registry(state: &mut ShellState, registry: WlRegistry, event: wl_registry::Event) {
    match event {
        wl_registry::Event::Global {
            name,
            interface,
            version,
        } => state.handle_global(&registry, name, &interface, version),
        wl_registry::Event::GlobalRemove { name } => state.handle_global_remove(name),
        _ => {}
    }
}

fn on_wl_output(state: &mut ShellState, output: WlOutput, event: wl_output::Event) {
    let incoming = state.screens.incoming(&output, event);
    state.follow_screen(incoming);
}

fn on_zxdg_output_v1(state: &mut ShellState, xdg: ZxdgOutputV1, event: zxdg_output_v1::Event) {
    let incoming = state.screens.handle_xdg_output(&xdg, event);
    state.follow_screen(incoming);
}

fn on_wl_surface(state: &mut ShellState, surface: WlSurface, event: wl_surface::Event) {
    match event {
        wl_surface::Event::Enter { output } => state.handle_surface_enter(&surface, output),
        wl_surface::Event::Leave { output } => state.handle_surface_leave(&surface, &output),
        wl_surface::Event::PreferredBufferScale { factor } => {
            state.handle_scale_factor(&surface, factor);
        }
        _ => {}
    }
}

fn on_wl_callback(state: &mut ShellState, callback: WlCallback, event: wl_callback::Event) {
    let wl_callback::Event::Done { .. } = event else {
        return;
    };
    if let Some(surface) = callback.key::<ObjectId>() {
        state.handle_frame_callback(&surface);
    } else if let Some(unmapping) = callback.key::<Unmapping>() {
        state.handle_unmapped(&unmapping.0);
    } else if let Some(sync) = callback.key::<UnmapSync>() {
        state.handle_unmap_sync(sync);
    }
}

fn on_zwlr_layer_surface_v1(
    state: &mut ShellState,
    proxy: ZwlrLayerSurfaceV1,
    event: zwlr_layer_surface_v1::Event,
) {
    let Some(surface) = proxy.key::<ObjectId>() else {
        return;
    };
    match event {
        zwlr_layer_surface_v1::Event::Configure {
            serial,
            width,
            height,
        } => state.handle_configure(&surface, serial, (width, height)),
        zwlr_layer_surface_v1::Event::Closed => state.handle_closed(&surface),
        _ => {}
    }
}

fn on_ext_session_lock_v1(
    state: &mut ShellState,
    _: ExtSessionLockV1,
    event: ext_session_lock_v1::Event,
) {
    match event {
        ext_session_lock_v1::Event::Locked => state.surfaces.handle_locked(),
        ext_session_lock_v1::Event::Finished => state.surfaces.handle_lock_finished(),
        _ => return,
    }
    state.reconcile();
}

fn on_ext_session_lock_surface_v1(
    state: &mut ShellState,
    proxy: ExtSessionLockSurfaceV1,
    event: ext_session_lock_surface_v1::Event,
) {
    let Some(surface) = proxy.key::<ObjectId>() else {
        return;
    };
    if let ext_session_lock_surface_v1::Event::Configure {
        serial,
        width,
        height,
    } = event
    {
        state.handle_configure(&surface, serial, (width, height));
    }
}

fn on_ext_idle_notification_v1(
    state: &mut ShellState,
    proxy: ExtIdleNotificationV1,
    event: ext_idle_notification_v1::Event,
) {
    let Some(index) = proxy.key::<u64>() else {
        return;
    };
    let index = index as usize;
    match event {
        ext_idle_notification_v1::Event::Idled => state.idling.idled(index, &mut state.host),
        ext_idle_notification_v1::Event::Resumed => {
            state.idling.resumed(index, &mut state.host);
        }
        _ => {}
    }
}
