use std::rc::Rc;
use std::sync::{Arc, Mutex};

use rikka_instance::LaunchRequest;
use rikka_swapchain::Presented;
use rikka_types::{PixelSize, Size};
use rikka_ui::{Command, Handler, Ui, WindowChrome, WindowRef, WindowRequest, request_window};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    GetClientRect, IsIconic, SW_MAXIMIZE, SW_MINIMIZE, SW_RESTORE, SetForegroundWindow,
    SetWindowTextW,
};
use windows::core::HSTRING;

use crate::sender::WindowAlive;
use crate::tasks::WindowId;
use crate::win32::drag::target::DropRegistration;
use crate::win32::input::grab::KeyboardGrab;
use crate::win32::input::raw::PointerLock;
use crate::win32::input::scale_of;
use crate::win32::monitor::Display;
use crate::win32::window::dwm::{self, Look};
use crate::win32::window::frame::is_maximized;
use crate::win32::window::handle::NativeWindow;
use crate::win32::window::placement::{self, Restore};
use crate::win32::window::shared::Shared;
use crate::window::{
    Closing, RequestedGeometry, Shaping, WindowHandlers, WindowShape, WindowTitle, set_chrome,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Modal {
    Move,
    Resize(rikka_types::ResizeEdge),
    Menu(rikka_types::Point),
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ModalCall {
    pub(crate) window: HWND,
    pub(crate) modal: Modal,
    pub(crate) maximized: bool,
    pub(crate) resizable: bool,
}

impl ModalCall {
    pub(crate) fn run(self) {
        match self.modal {
            Modal::Move => placement::start_move(self.window),
            Modal::Resize(edge) => placement::start_resize(self.window, edge),
            Modal::Menu(at) => {
                placement::show_menu(self.window, at, self.maximized, self.resizable);
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Measured {
    pub(crate) pixels: PixelSize,
    pub(crate) scale: f64,
}

impl Measured {
    pub(crate) fn logical(self) -> Size {
        Size::new(
            (f64::from(self.pixels.width) / self.scale) as f32,
            (f64::from(self.pixels.height) / self.scale) as f32,
        )
    }
}

#[derive(Debug)]
pub(crate) struct DisabledOwner(HWND);

impl DisabledOwner {
    pub(crate) fn disable(owner: HWND) -> DisabledOwner {
        let _was_disabled = unsafe { EnableWindow(owner, false) };
        DisabledOwner(owner)
    }
}

impl Drop for DisabledOwner {
    fn drop(&mut self) {
        let _was_disabled = unsafe { EnableWindow(self.0, true) };
        if !unsafe { SetForegroundWindow(self.0) }.as_bool() {
            tracing::debug!("owner window did not come forward");
        }
    }
}

pub(crate) struct WindowState {
    pub(crate) id: WindowId,
    pub(crate) alive: WindowAlive,
    pub(crate) chrome: WindowChrome,
    pub(crate) close_requested: bool,
    pub(crate) closing: Closing,
    pub(crate) presented: Arc<Mutex<Option<Presented>>>,
    pub(crate) display: Display,
    focus: Option<Handler>,
    blur: Option<Handler>,
    requested: RequestedGeometry,
    window_title: WindowTitle,
    shape: Option<Handler<(WindowShape,)>>,
    open_handler: Option<Handler<(LaunchRequest,)>>,
    shaping: Shaping,
    raising: Vec<WindowRef>,
    modals: Vec<Modal>,
    quitting: bool,
    measured: Option<Measured>,
    maximized: bool,
    minimized: bool,
    fullscreen: Option<Restore>,
    focused: bool,
    shown: bool,
    painted: bool,
    withheld: bool,
    look: Option<Look>,
    locked: Option<PointerLock>,
    grab: Option<KeyboardGrab>,
    _owner: Option<DisabledOwner>,
    _drop_target: Option<DropRegistration>,
    native: NativeWindow,
}

pub(crate) struct Parts {
    pub(crate) id: WindowId,
    pub(crate) native: NativeWindow,
    pub(crate) alive: WindowAlive,
    pub(crate) chrome: WindowChrome,
    pub(crate) hooks: WindowHandlers,
    pub(crate) requested: RequestedGeometry,
    pub(crate) owner: Option<DisabledOwner>,
    pub(crate) drop_target: Option<DropRegistration>,
    pub(crate) shown: bool,
}

impl WindowState {
    pub(crate) fn new(parts: Parts) -> Self {
        let Parts {
            id,
            native,
            alive,
            chrome,
            hooks,
            requested,
            owner,
            drop_target,
            shown,
        } = parts;
        let display = Display::of(native.hwnd());
        Self {
            id,
            alive,
            chrome,
            close_requested: false,
            closing: hooks.closing,
            presented: Arc::new(Mutex::new(None)),
            display,
            focus: hooks.focus,
            blur: hooks.blur,
            requested,
            window_title: hooks.window_title,
            shape: hooks.shape,
            open_handler: hooks.open_handler,
            shaping: Shaping::new(requested.size),
            raising: Vec::new(),
            modals: Vec::new(),
            quitting: false,
            measured: None,
            maximized: false,
            minimized: false,
            fullscreen: None,
            focused: false,
            shown,
            painted: false,
            withheld: false,
            look: None,
            locked: None,
            grab: None,
            _owner: owner,
            _drop_target: drop_target,
            native,
        }
    }

    pub(crate) fn hwnd(&self) -> HWND {
        self.native.hwnd()
    }

    pub(crate) fn shared(&self) -> &Rc<Shared> {
        self.native.shared()
    }

    pub(crate) fn measure(&mut self) -> bool {
        let window = self.hwnd();
        self.maximized = is_maximized(window);
        self.minimized = unsafe { IsIconic(window) }.as_bool();
        let mut client = RECT::default();
        if let Err(error) = unsafe { GetClientRect(window, &mut client) } {
            tracing::warn!("failed to measure window: {error}");
            return false;
        }
        let pixels = PixelSize::new(
            u32::try_from(client.right - client.left).unwrap_or(0),
            u32::try_from(client.bottom - client.top).unwrap_or(0),
        );
        if self.minimized || pixels.is_empty() {
            return false;
        }
        let measured = Measured {
            pixels,
            scale: scale_of(window),
        };
        let changed = self.measured != Some(measured);
        self.measured = Some(measured);
        self.display = Display::of(window);
        changed
    }

    pub(crate) fn measured(&self) -> Option<Measured> {
        self.measured
    }

    pub(crate) fn is_drawable(&self) -> bool {
        self.measured.is_some() && !self.minimized && !self.withheld
    }

    pub(crate) fn withhold(&mut self, withheld: bool) {
        self.withheld = withheld;
    }

    pub(crate) fn present(&mut self) {
        self.painted = true;
    }

    pub(crate) fn take_first_show(&mut self) -> bool {
        let first = self.painted && !self.shown && !self.withheld;
        if first {
            self.shown = true;
        }
        first
    }

    pub(crate) fn follow_look(&mut self, look: Look) {
        if self.look == Some(look) {
            return;
        }
        self.look = Some(look);
        dwm::apply(self.hwnd(), look);
    }

    pub(crate) fn opened(&self, open: LaunchRequest) -> Option<Handler> {
        let handler = self.open_handler.clone()?;
        Some(handler.map(move |()| (open.clone(),)))
    }

    pub(crate) fn keyboard(&mut self, entered: bool) -> Option<Handler> {
        if std::mem::replace(&mut self.focused, entered) == entered {
            return None;
        }
        if let Some(locked) = self.locked.as_mut() {
            locked.follow_focus(entered);
        }
        match entered {
            true => self.focus.clone(),
            false => self.blur.clone(),
        }
    }

    pub(crate) fn is_focused(&self) -> bool {
        self.focused
    }

    pub(crate) fn is_locked(&self) -> bool {
        self.locked.is_some()
    }

    pub(crate) fn service(&mut self, ui: &mut Ui) -> bool {
        self.sync_title(ui);
        self.sync_shape(ui);
        self.sync_chrome(ui);
        self.sync_lock(ui);
        self.sync_grab(ui);
        self.serve_unclaimed(ui);
        self.apply_window_requests(ui)
    }

    pub(crate) fn take_raising(&mut self) -> Vec<WindowRef> {
        std::mem::take(&mut self.raising)
    }

    pub(crate) fn take_modals(&mut self) -> Vec<Modal> {
        std::mem::take(&mut self.modals)
    }

    pub(crate) fn take_quitting(&mut self) -> bool {
        std::mem::take(&mut self.quitting)
    }

    pub(crate) fn is_maximized(&self) -> bool {
        self.maximized
    }

    pub(crate) fn is_resizable(&self) -> bool {
        self.requested.resizable
    }

    fn serve_unclaimed(&mut self, ui: &mut Ui) {
        match ui.take_unclaimed_command() {
            Some(Command::Close) => ui.start(|| request_window(WindowRequest::AskToClose)),
            Some(Command::Quit) => self.quitting = true,
            Some(Command::Fullscreen) => {
                ui.start(|| request_window(WindowRequest::ToggleFullscreen));
            }
            _ => {}
        }
    }

    pub(crate) fn sync_chrome(&mut self, ui: &mut Ui) {
        let current = (
            self.focused,
            self.maximized,
            self.fullscreen.is_some(),
            self.requested.resizable,
        );
        let shown = (
            self.chrome.focused,
            self.chrome.maximized,
            self.chrome.fullscreen,
            self.chrome.resizable,
        );
        if current == shown {
            return;
        }
        (
            self.chrome.focused,
            self.chrome.maximized,
            self.chrome.fullscreen,
            self.chrome.resizable,
        ) = current;
        set_chrome(ui, self.chrome.clone());
    }

    fn sync_shape(&mut self, ui: &mut Ui) {
        let Some(measured) = self.measured else {
            return;
        };
        let Some(shape) = self.shaping.update(
            measured.logical(),
            self.maximized,
            self.fullscreen.is_some(),
        ) else {
            return;
        };
        if let Some(hook) = &self.shape {
            ui.start(hook.clone().map(move |()| (shape,)));
        }
    }

    fn sync_title(&mut self, ui: &mut Ui) {
        let Some(title) = self.window_title.take() else {
            return;
        };
        if title == self.chrome.title {
            return;
        }
        if let Err(error) = unsafe { SetWindowTextW(self.hwnd(), &HSTRING::from(title.as_str())) } {
            tracing::warn!("failed to set window title: {error}");
        }
        self.chrome.title = title;
        set_chrome(ui, self.chrome.clone());
    }

    fn sync_lock(&mut self, ui: &Ui) {
        let requested = ui.pointer_locked();
        if requested == self.locked.is_some() {
            return;
        }
        self.locked = match requested {
            true => PointerLock::hold(self.hwnd()).map(|mut locked| {
                locked.follow_focus(self.focused);
                locked
            }),
            false => None,
        };
    }

    fn sync_grab(&mut self, ui: &Ui) {
        let requested = ui.grabs_keys() && self.focused;
        if requested == self.grab.is_some() {
            return;
        }
        self.grab = match requested {
            true => KeyboardGrab::hold(self.hwnd(), self.id),
            false => None,
        };
    }

    fn apply_window_requests(&mut self, ui: &mut Ui) -> bool {
        let mut requests = Vec::new();
        let closes = self.closing.serve(ui, |request| requests.push(request));
        for request in requests {
            self.apply(request);
        }
        closes
    }

    fn apply(&mut self, request: WindowRequest) {
        let window = self.hwnd();
        let floating = !self.maximized && self.fullscreen.is_none();
        match request {
            WindowRequest::Move if self.fullscreen.is_none() => self.modals.push(Modal::Move),
            WindowRequest::Resize(edge) if floating && self.requested.resizable => {
                self.modals.push(Modal::Resize(edge));
            }
            WindowRequest::Move | WindowRequest::Resize(_) => {}
            WindowRequest::Minimize => {
                placement::show(window, SW_MINIMIZE);
            }
            WindowRequest::ToggleMaximize if self.requested.resizable => {
                let command = match self.maximized {
                    true => SW_RESTORE,
                    false => SW_MAXIMIZE,
                };
                placement::show(window, command);
            }
            WindowRequest::ToggleMaximize => {}
            WindowRequest::ShowMenu(at) => self.modals.push(Modal::Menu(at)),
            WindowRequest::Fullscreen(on) => self.set_fullscreen(on),
            WindowRequest::ToggleFullscreen => {
                let on = self.fullscreen.is_none();
                self.set_fullscreen(on);
            }
            WindowRequest::Activate(_) => bring_forward(window),
            WindowRequest::Raise(target) => self.raising.push(target),
            WindowRequest::AskToClose | WindowRequest::Close => {}
        }
    }

    fn set_fullscreen(&mut self, on: bool) {
        if on == self.fullscreen.is_some() {
            return;
        }
        let shared = Rc::clone(self.shared());
        let mut frame = shared.frame.get();
        frame.fullscreen = on;
        shared.frame.set(frame);
        match on {
            true => {
                self.fullscreen = placement::enter_fullscreen(self.hwnd());
                if self.fullscreen.is_none() {
                    frame.fullscreen = false;
                    shared.frame.set(frame);
                }
            }
            false => {
                if let Some(restore) = self.fullscreen.take() {
                    placement::leave_fullscreen(self.hwnd(), &restore);
                }
            }
        }
    }
}

pub(crate) fn bring_forward(window: HWND) {
    if unsafe { IsIconic(window) }.as_bool() {
        placement::show(window, SW_RESTORE);
    }
    if !unsafe { SetForegroundWindow(window) }.as_bool() {
        tracing::debug!("window did not come forward");
    }
}
