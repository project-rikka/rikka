use rikka_wayland_client::wl_callback::{self, WlCallback};
use rikka_wayland_client::wl_output::{self, WlOutput};
use rikka_wayland_client::wl_registry::{self, WlRegistry};
use rikka_wayland_client::wl_seat::WlSeat;
use rikka_wayland_client::wl_surface::{self, WlSurface};
use rikka_wayland_client::wp_fractional_scale_v1::{self, WpFractionalScaleV1};
use rikka_wayland_client::xdg_surface::{self, XdgSurface};
use rikka_wayland_client::xdg_toplevel::{self, XdgToplevel};
use rikka_wayland_client::xdg_wm_base::{self, XdgWmBase};
use rikka_wayland_client::zwp_locked_pointer_v1::{self, ZwpLockedPointerV1};
use rikka_wayland_client::zxdg_exported_v2::{self, ZxdgExportedV2};
use rikka_wayland_client::zxdg_imported_v2::{self, ZxdgImportedV2};
use rikka_wayland_client::zxdg_output_v1::{self, ZxdgOutputV1};
use rikka_wayland_client::zxdg_toplevel_decoration_v1::{self, ZxdgToplevelDecorationV1};

use rikka_seat::Seats;
use rikka_types::WindowIdentifier;
use rikka_wayland_client::{Proxy as _, Queue};

use crate::tasks::WindowId;
use crate::wayland::runner::Runner;

pub(crate) fn listen(queue: &mut Queue<Runner>) {
    Seats::listen(queue);
    queue.on::<WlRegistry>(on_wl_registry);
    queue.on::<ZxdgOutputV1>(on_zxdg_output_v1);
    queue.on::<WlOutput>(on_wl_output);
    queue.on::<XdgWmBase>(on_xdg_wm_base);
    queue.on::<XdgSurface>(on_xdg_surface);
    queue.on::<XdgToplevel>(on_xdg_toplevel);
    queue.on::<ZxdgToplevelDecorationV1>(on_zxdg_toplevel_decoration_v1);
    queue.on::<WpFractionalScaleV1>(on_wp_fractional_scale_v1);
    queue.on::<WlSurface>(on_wl_surface);
    queue.on::<WlCallback>(on_wl_callback);
    queue.on::<ZwpLockedPointerV1>(on_zwp_locked_pointer_v1);
    queue.on::<ZxdgExportedV2>(on_zxdg_exported_v2);
    queue.on::<ZxdgImportedV2>(on_zxdg_imported_v2);
}

fn on_wl_registry(runner: &mut Runner, registry: WlRegistry, event: wl_registry::Event) {
    match event {
        wl_registry::Event::Global {
            name,
            interface,
            version,
        } => {
            if interface == WlOutput::INTERFACE.name {
                let xdg_output_manager = runner.client.globals.described_outputs.as_ref();
                runner
                    .client
                    .outputs
                    .add(&registry, (name, version), xdg_output_manager);
            } else if interface == WlSeat::INTERFACE.name {
                runner.client.seats.add(&registry, name, version);
            }
        }
        wl_registry::Event::GlobalRemove { name } => {
            if let Some(output) = runner.client.outputs.remove(name) {
                runner.forget_output(&output);
            } else {
                runner.client.seats.remove(name);
            }
        }
        _ => {}
    }
}

fn on_zxdg_output_v1(runner: &mut Runner, proxy: ZxdgOutputV1, event: zxdg_output_v1::Event) {
    let Some(global) = proxy.key::<u32>() else {
        return;
    };
    runner.client.outputs.handle_xdg_event(global, event);
}

fn on_wl_output(runner: &mut Runner, output: WlOutput, event: wl_output::Event) {
    if runner.client.outputs.handle_event(&output, event) {
        runner.screens_moved = true;
    }
}

fn on_xdg_wm_base(_: &mut Runner, wm_base: XdgWmBase, event: xdg_wm_base::Event) {
    if let xdg_wm_base::Event::Ping { serial } = event {
        wm_base.pong(serial);
    }
}

fn on_xdg_surface(runner: &mut Runner, proxy: XdgSurface, event: xdg_surface::Event) {
    let Some(id) = proxy.key::<WindowId>() else {
        return;
    };
    let xdg_surface::Event::Configure { serial } = event else {
        return;
    };
    if let Some(window) = runner.window_mut(id) {
        window.arrived = Some(window.incoming.done(serial));
    }
}

fn on_xdg_toplevel(runner: &mut Runner, proxy: XdgToplevel, event: xdg_toplevel::Event) {
    let Some(id) = proxy.key::<WindowId>() else {
        return;
    };
    let Some(window) = runner.window_mut(id) else {
        return;
    };
    match event {
        xdg_toplevel::Event::Configure {
            width,
            height,
            states,
        } => window.incoming.toplevel(width, height, &states),
        xdg_toplevel::Event::ConfigureBounds { width, height } => {
            window.incoming.bounded(width, height);
        }
        xdg_toplevel::Event::Close => window.close_requested = true,
        _ => {}
    }
}

fn on_zxdg_toplevel_decoration_v1(
    runner: &mut Runner,
    proxy: ZxdgToplevelDecorationV1,
    event: zxdg_toplevel_decoration_v1::Event,
) {
    let Some(id) = proxy.key::<WindowId>() else {
        return;
    };
    if let zxdg_toplevel_decoration_v1::Event::Configure { mode } = event
        && let Some(window) = runner.window_mut(id)
    {
        window.incoming.decorated(mode);
    }
}

fn on_wp_fractional_scale_v1(
    runner: &mut Runner,
    proxy: WpFractionalScaleV1,
    event: wp_fractional_scale_v1::Event,
) {
    let Some(id) = proxy.key::<WindowId>() else {
        return;
    };
    if let Some(window) = runner.window_mut(id) {
        window.toplevel.scaling_mut().handle_event(event);
    }
}

fn on_wl_surface(runner: &mut Runner, proxy: WlSurface, event: wl_surface::Event) {
    let Some(id) = proxy.key::<WindowId>() else {
        return;
    };
    let Some(window) = runner.window_mut(id) else {
        return;
    };
    let moved = match event {
        wl_surface::Event::Enter { output } => {
            window.outputs.push(output);
            true
        }
        wl_surface::Event::Leave { output } => {
            window.outputs.retain(|entered| entered != &output);
            true
        }
        wl_surface::Event::PreferredBufferScale { factor } => {
            window
                .toplevel
                .scaling_mut()
                .set_preferred_buffer_scale(factor);
            false
        }
        _ => false,
    };
    runner.screens_moved |= moved;
}

fn on_wl_callback(runner: &mut Runner, proxy: WlCallback, event: wl_callback::Event) {
    let Some(id) = proxy.key::<WindowId>() else {
        return;
    };
    if let wl_callback::Event::Done { .. } = event {
        runner.frame_answered(id);
    }
}

fn on_zwp_locked_pointer_v1(
    _: &mut Runner,
    _: ZwpLockedPointerV1,
    event: zwp_locked_pointer_v1::Event,
) {
    if let zwp_locked_pointer_v1::Event::Unlocked = event {
        tracing::debug!("pointer unlocked by compositor");
    }
}

fn on_zxdg_exported_v2(runner: &mut Runner, proxy: ZxdgExportedV2, event: zxdg_exported_v2::Event) {
    let Some(id) = proxy.key::<WindowId>() else {
        return;
    };
    if let zxdg_exported_v2::Event::Handle { handle } = event {
        runner.identify(id, WindowIdentifier::Wayland(handle));
    }
}

fn on_zxdg_imported_v2(runner: &mut Runner, proxy: ZxdgImportedV2, event: zxdg_imported_v2::Event) {
    let Some(id) = proxy.key::<WindowId>() else {
        return;
    };
    if let zxdg_imported_v2::Event::Destroyed = event
        && let Some(window) = runner.window_mut(id)
    {
        tracing::debug!("parent window is gone");
        window.toplevel.forget_parent();
    }
}
