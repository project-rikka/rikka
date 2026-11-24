use std::rc::Rc;
use std::sync::Arc;

use rikka_theme::Theme;
use rikka_types::{Size, WindowIdentifier};
use rikka_ui::WindowChrome;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowRect, SW_SHOWMAXIMIZED, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos, WINDOW_EX_STYLE,
    WINDOW_STYLE, WS_EX_APPWINDOW, WS_MAXIMIZEBOX, WS_OVERLAPPEDWINDOW,
};

use crate::error::Result;
use crate::sender::{Route, WindowAlive};
use crate::tasks::{LoopProxy, WindowId};
use crate::win32::drag::target;
use crate::win32::hosting::Hosting;
use crate::win32::input::cursor::Pointer;
use crate::win32::monitor::Display;
use crate::win32::window::dwm::Look;
use crate::win32::window::handle::{Creation, NativeWindow, WindowClass};
use crate::win32::window::shared::{Frame, Shared};
use crate::win32::window::state::{DisabledOwner, Parts, WindowState};
use crate::win32::window::{frame, placement};
use crate::window::{BuiltWindow, Opening, RequestedGeometry, WindowParent, set_chrome};

pub(crate) struct Opener<'a> {
    pub(crate) class: WindowClass,
    pub(crate) proxy: &'a LoopProxy,
    pub(crate) theme: &'a Theme,
    pub(crate) pointer: Pointer,
}

pub(crate) fn open(
    hosting: &mut Hosting,
    id: WindowId,
    opener: &Opener<'_>,
    mut opening: Opening,
) -> Result<()> {
    opening.look.title = opening.look.title.retranslated();
    let look = &opening.look;
    let client_side = !look.server_decorations;
    if look.transparent {
        tracing::warn!("transparent windows are unsupported on Windows; drawing it opaque");
    }
    let owner = look
        .parent
        .as_ref()
        .and_then(|parent| owner_of(hosting, parent));
    let requested = RequestedGeometry {
        size: look.size,
        min_size: look.min_size,
        resizable: look.resizable,
    };
    let shared = Rc::new(Shared::new(
        id,
        Frame {
            custom: client_side,
            resizable: requested.resizable,
            fullscreen: false,
            min_size: match requested.resizable {
                true => requested.min_size,
                false => Some(requested.size),
            },
        },
        opener.pointer,
    ));
    let style = match look.resizable {
        true => WS_OVERLAPPEDWINDOW,
        false => WINDOW_STYLE(WS_OVERLAPPEDWINDOW.0 & !WS_MAXIMIZEBOX.0),
    };
    let extended = match owner {
        Some(_) => WINDOW_EX_STYLE::default(),
        None => WS_EX_APPWINDOW,
    };
    let native = NativeWindow::create(
        opener.class,
        Rc::clone(&shared),
        &Creation {
            title: look.title.as_str(),
            style,
            extended,
            owner,
        },
    )?;
    let window = native.hwnd();
    place(&shared, window, look.size, owner);
    if look.maximized && look.resizable {
        placement::show(window, SW_SHOWMAXIMIZED);
    }
    let drop_target = target::register(window, id);
    let modality = (look.modal)
        .then_some(owner)
        .flatten()
        .map(DisabledOwner::disable);
    let chrome = WindowChrome::new(look.title.clone());
    let hooks = std::mem::take(&mut opening.hooks);
    let alive = WindowAlive::open();
    let watched = alive.watch();
    let maximized = look.maximized && look.resizable;
    let mut ui = opening.into_ui(BuiltWindow {
        theme: opener.theme.clone(),
        client_side,
        route: Route::Window {
            proxy: opener.proxy.clone(),
            window: id,
        },
        alive: watched,
    });
    ui.set_window_identifier(Some(WindowIdentifier::Win32(window.0 as usize)));
    let mut state = WindowState::new(Parts {
        id,
        native,
        alive,
        chrome,
        hooks,
        requested,
        owner: modality,
        drop_target,
        shown: maximized,
    });
    state.measure();
    if let Some(measured) = state.measured() {
        ui.set_viewport(measured.logical(), measured.scale as f32);
    }
    state.follow_look(Look::of(opener.theme));
    let hosted = hosting.insert(id, ui, state);
    hosted.drawable(false);
    hosted.presents_with(Arc::new(WindowState::present));
    set_chrome(&mut hosted.ui, hosted.state.chrome.clone());
    hosted.state.sync_chrome(&mut hosted.ui);
    Ok(())
}

fn owner_of(hosting: &Hosting, parent: &WindowParent) -> Option<HWND> {
    match parent {
        WindowParent::Window(window) => {
            let hosted = hosting.get(&WindowId::from(window.get()));
            if hosted.is_none() {
                tracing::warn!("no window matches parent {window:?}");
            }
            hosted.map(|hosted| hosted.state.hwnd())
        }
        WindowParent::Foreign(WindowIdentifier::Win32(window)) => Some(HWND(*window as *mut _)),
        WindowParent::Foreign(
            identifier @ (WindowIdentifier::Wayland(_) | WindowIdentifier::X11(_)),
        ) => {
            tracing::warn!("cannot parent a windows window to {identifier}");
            None
        }
    }
}

fn place(shared: &Shared, window: HWND, client: Size, owner: Option<HWND>) {
    let display = Display::of(owner.unwrap_or(window));
    let Some(work) = display.work_area() else {
        return;
    };
    let (width, height) = frame::outer_size(shared, window, client);
    let width = width.min(work.right - work.left);
    let height = height.min(work.bottom - work.top);
    let mut current = RECT::default();
    if let Err(error) = unsafe { GetWindowRect(window, &mut current) } {
        tracing::debug!("failed to read the new window position: {error}");
    }
    let (left, top) = match owner.and_then(rect_of) {
        Some(around) => (
            around.left + (around.right - around.left - width) / 2,
            around.top + (around.bottom - around.top - height) / 2,
        ),
        None => (current.left, current.top),
    };
    let left = left.clamp(work.left, (work.right - width).max(work.left));
    let top = top.clamp(work.top, (work.bottom - height).max(work.top));
    if let Err(error) = unsafe {
        SetWindowPos(
            window,
            None,
            left,
            top,
            width,
            height,
            SWP_NOZORDER | SWP_NOACTIVATE,
        )
    } {
        tracing::warn!("failed to size the new window: {error}");
    }
}

fn rect_of(window: HWND) -> Option<RECT> {
    let mut rect = RECT::default();
    unsafe { GetWindowRect(window, &mut rect) }.ok()?;
    Some(rect)
}
