use std::sync::Arc;

use rikka_theme::Theme;
use rikka_types::WindowIdentifier;
use rikka_ui::WindowChrome;

use crate::sender::{Route, WindowAlive};
use crate::tasks::{LoopProxy, WindowId};
use crate::wayland::hosting::Windows;
use crate::wayland::toplevel::{ParentToplevel, Toplevel, ToplevelConfig};
use crate::wayland::window_state::{Parts, WindowState};
use crate::window::{BuiltWindow, Opening, RequestedGeometry, WindowParent, set_chrome};

pub(crate) fn open(
    windows: &mut Windows<'_>,
    id: WindowId,
    proxy: &LoopProxy,
    app_name: &str,
    theme: &Theme,
    mut opening: Opening,
) {
    opening.look.title = opening.look.title.retranslated();
    let look = &opening.look;
    let glaze = theme.translucency();
    let client_side = !look.server_decorations;
    let needs_alpha = look.transparent;
    let transparent = needs_alpha || glaze.is_translucent();
    let toplevel = Toplevel::open(
        &windows.client.globals,
        id,
        &ToplevelConfig {
            app_id: app_name,
            title: look.title.as_str(),
            min_size: look.min_size,
            fixed: (!look.resizable).then_some(look.size),
            maximized: look.maximized,
            server_decorations: look.server_decorations,
            expected_scale: windows.client.outputs.expected_scale(),
            parent: look
                .parent
                .as_ref()
                .and_then(|parent| parent_toplevel(windows, parent)),
            modal: look.modal,
        },
    );
    if let Some(token) = &look.token {
        windows.session.activate(toplevel.surface(), token);
    }
    let requested = RequestedGeometry {
        size: look.size,
        min_size: look.min_size,
        resizable: look.resizable,
    };
    let chrome = WindowChrome::new(look.title.clone());
    let glazing = windows.session.glaze(
        toplevel.surface(),
        glaze.blur_radius().is_some(),
        !look.transparent,
    );
    let feedback = windows.session.feedback(toplevel.surface(), id);
    let hooks = std::mem::take(&mut opening.hooks);
    let alive = WindowAlive::open();
    let watched = alive.watch();
    let mut ui = opening.into_ui(BuiltWindow {
        theme: theme.clone(),
        client_side,
        route: Route::Window {
            proxy: proxy.clone(),
            window: id,
        },
        alive: watched,
    });
    ui.set_viewport(requested.size, 1.0);
    let state = WindowState::new(Parts {
        id,
        toplevel,
        alive,
        chrome,
        glazing,
        needs_alpha,
        transparent,
        client_side,
        hooks,
        requested,
        feedback,
    });
    let hosted = windows.hosting.insert(id, ui, state);
    hosted.drawable(false);
    hosted.seals_with(Arc::new(WindowState::seal));
    hosted.presents_with(Arc::new(WindowState::present));
    set_chrome(&mut hosted.ui, hosted.state.chrome.clone());
    hosted.state.sync_background(&mut hosted.ui);
    hosted.state.sync_chrome(&mut hosted.ui);
}

fn parent_toplevel<'a>(
    windows: &'a Windows<'_>,
    parent: &'a WindowParent,
) -> Option<ParentToplevel<'a>> {
    match parent {
        WindowParent::Window(window) => {
            let hosted = windows.hosting.get(&WindowId::from(window.get()));
            if hosted.is_none() {
                tracing::warn!("no window matches parent {window:?}");
            }
            hosted.map(|hosted| ParentToplevel::Ours(hosted.state.toplevel.xdg_toplevel()))
        }
        WindowParent::Foreign(WindowIdentifier::Wayland(handle)) => {
            Some(ParentToplevel::Exported(handle))
        }
        WindowParent::Foreign(
            identifier @ (WindowIdentifier::X11(_) | WindowIdentifier::Win32(_)),
        ) => {
            tracing::warn!("cannot parent a wayland window to {identifier}");
            None
        }
    }
}
