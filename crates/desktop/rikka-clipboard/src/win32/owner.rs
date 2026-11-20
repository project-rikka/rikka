use std::cell::RefCell;
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::sync::{Mutex, OnceLock};
use std::thread::{self, JoinHandle};

use windows::Win32::Foundation::{
    ERROR_CLASS_ALREADY_EXISTS, HINSTANCE, HWND, LPARAM, LRESULT, SetLastError, WIN32_ERROR, WPARAM,
};
use windows::Win32::System::DataExchange::{
    AddClipboardFormatListener, EmptyClipboard, GetClipboardOwner, RemoveClipboardFormatListener,
    SetClipboardData,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, HWND_MESSAGE,
    MSG, PostMessageW, PostQuitMessage, RegisterClassExW, WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP,
    WM_CLIPBOARDUPDATE, WM_DESTROY, WM_DESTROYCLIPBOARD, WM_RENDERALLFORMATS, WM_RENDERFORMAT,
    WNDCLASSEXW,
};
use windows::core::{HRESULT, PCWSTR, w};

use rikka_clipboard_core::{Error, Offer, Result, Selection};

use crate::watch::{Change, Watcher};
use crate::win32::formats::{self, Opened, Registered};
use crate::win32::global::{Global, io_error};

const CLASS: PCWSTR = w!("RikkaClipboardOwner");
const WAKE: u32 = WM_APP + 1;
const STOP: u32 = WM_APP + 2;

pub(super) enum Request {
    Publish {
        selection: Selection,
        offer: Offer,
        by: u64,
    },
    Watch {
        by: u64,
        watcher: Watcher,
    },
    MimeTypes {
        selection: Selection,
        reply: SyncSender<Result<Vec<String>>>,
    },
    Read {
        selection: Selection,
        mime: String,
        limit: usize,
        reply: SyncSender<Result<Vec<u8>>>,
    },
}

pub(super) struct Owner {
    requests: Sender<Request>,
    window: isize,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl Owner {
    pub(super) fn start() -> Result<Owner> {
        let (requests, received) = mpsc::channel();
        let (opened, window) = mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name("rikka-clipboard".to_owned())
            .spawn(move || run(received, &opened))
            .map_err(Error::Io)?;
        let window = window
            .recv()
            .map_err(|_| Error::Unavailable("clipboard thread exited during startup"))??;
        Ok(Owner {
            requests,
            window,
            thread: Mutex::new(Some(thread)),
        })
    }

    pub(super) fn send(&self, request: Request) -> Result<()> {
        self.requests
            .send(request)
            .map_err(|_| Error::Unavailable("clipboard worker stopped"))?;
        self.post(WAKE)
    }

    pub(super) fn stop(&self) {
        let Some(thread) = self.thread.lock().ok().and_then(|mut held| held.take()) else {
            return;
        };
        if let Err(error) = self.post(STOP) {
            tracing::debug!("failed to stop clipboard worker: {error}");
            return;
        }
        if thread.join().is_err() {
            tracing::warn!("clipboard worker panicked");
        }
    }

    fn post(&self, message: u32) -> Result<()> {
        unsafe {
            PostMessageW(
                Some(HWND(self.window as *mut _)),
                message,
                WPARAM(0),
                LPARAM(0),
            )
        }
        .map_err(io_error)
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        self.stop();
    }
}

impl std::fmt::Debug for Owner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Owner").finish_non_exhaustive()
    }
}

struct Published {
    offer: Offer,
    by: u64,
}

struct State {
    window: HWND,
    requests: Receiver<Request>,
    registered: Registered,
    published: Option<Published>,
    primary: Option<Published>,
    watchers: Vec<(u64, Watcher)>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

fn with_state<T>(act: impl FnOnce(&mut State) -> T) -> Option<T> {
    STATE.with(|state| match state.try_borrow_mut() {
        Ok(mut state) => state.as_mut().map(act),
        Err(_) => {
            tracing::error!("clipboard state reentered");
            None
        }
    })
}

fn run(requests: Receiver<Request>, opened: &SyncSender<Result<isize>>) {
    let window = match open_window() {
        Ok(window) => window,
        Err(error) => {
            drop(opened.send(Err(error)));
            return;
        }
    };
    STATE.with(|state| {
        *state.borrow_mut() = Some(State {
            window,
            requests,
            registered: Registered::new(),
            published: None,
            primary: None,
            watchers: Vec::new(),
        });
    });
    if let Err(error) = unsafe { AddClipboardFormatListener(window) } {
        tracing::warn!("clipboard changes unavailable: {error}");
    }
    if opened.send(Ok(window.0 as isize)).is_err() {
        return;
    }
    let mut message = MSG::default();
    loop {
        let got = unsafe { GetMessageW(&mut message, None, 0, 0) };
        match got.0 {
            0 => break,
            -1 => {
                tracing::error!(
                    "clipboard message loop failed: {}",
                    windows::core::Error::from_thread()
                );
                break;
            }
            _ => unsafe {
                DispatchMessageW(&message);
            },
        }
    }
    STATE.with(|state| state.borrow_mut().take());
}

fn open_window() -> Result<HWND> {
    static CLASS_REGISTERED: OnceLock<Result<(), HRESULT>> = OnceLock::new();
    let instance = unsafe { GetModuleHandleW(None) }.map_err(io_error)?;
    let instance = HINSTANCE(instance.0);
    let registered = CLASS_REGISTERED.get_or_init(|| {
        let class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(procedure),
            hInstance: instance,
            lpszClassName: CLASS,
            ..WNDCLASSEXW::default()
        };
        match unsafe { RegisterClassExW(&class) } {
            0 => match windows::core::Error::from_thread().code() {
                code if code == ERROR_CLASS_ALREADY_EXISTS.to_hresult() => Ok(()),
                code => Err(code),
            },
            _ => Ok(()),
        }
    });
    if let Err(code) = registered {
        return Err(io_error(windows::core::Error::from_hresult(*code)));
    }
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            CLASS,
            w!(""),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(instance),
            None,
        )
    }
    .map_err(io_error)
}

unsafe extern "system" fn procedure(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WAKE => {
            serve_requests();
            LRESULT(0)
        }
        STOP => {
            if let Err(error) = unsafe { RemoveClipboardFormatListener(window) } {
                tracing::debug!("failed to stop watching the clipboard: {error}");
            }
            if let Err(error) = unsafe { DestroyWindow(window) } {
                tracing::error!("failed to destroy clipboard window: {error}");
                unsafe { PostQuitMessage(0) };
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        WM_CLIPBOARDUPDATE => {
            announce_clipboard();
            LRESULT(0)
        }
        WM_RENDERFORMAT => {
            render(wparam.0 as u32);
            LRESULT(0)
        }
        WM_RENDERALLFORMATS => {
            render_all();
            LRESULT(0)
        }
        WM_DESTROYCLIPBOARD => {
            with_state(|state| state.published = None);
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}

fn serve_requests() {
    while let Some(request) = with_state(|state| state.requests.try_recv().ok()).flatten() {
        serve(request);
    }
}

fn serve(request: Request) {
    match request {
        Request::Publish {
            selection: Selection::Clipboard,
            offer,
            by,
        } => {
            if let Err(error) = publish(offer, by) {
                tracing::warn!("failed to publish clipboard: {error}");
            }
        }
        Request::Publish {
            selection: Selection::Primary,
            offer,
            by,
        } => publish_primary(offer, by),
        Request::Watch { by, watcher } => {
            with_state(|state| state.watchers.push((by, watcher)));
        }
        Request::MimeTypes { selection, reply } => {
            drop(reply.send(mime_types(selection)));
        }
        Request::Read {
            selection,
            mime,
            limit,
            reply,
        } => drop(reply.send(read(selection, &mime, limit))),
    }
}

fn publish(offer: Offer, by: u64) -> Result<()> {
    let Some((window, registered)) = with_state(|state| (state.window, state.registered)) else {
        return Err(Error::Unavailable("clipboard worker stopped"));
    };
    let formats = formats::publishable(&offer, &registered);
    let opened = Opened::new(window)?;
    unsafe { EmptyClipboard() }.map_err(io_error)?;
    if offer.is_empty() {
        with_state(|state| state.published = None);
        return Ok(());
    }
    with_state(|state| state.published = Some(Published { offer, by }));
    for format in formats {
        unsafe { SetLastError(WIN32_ERROR(0)) };
        if let Err(error) = unsafe { SetClipboardData(format, None) }
            && error.code().is_err()
        {
            tracing::warn!("failed to offer clipboard format {format}: {error}");
        }
    }
    drop(opened);
    Ok(())
}

fn publish_primary(offer: Offer, by: u64) {
    let mime_types: Vec<String> = offer.mime_types().map(str::to_owned).collect();
    let owner = (!offer.is_empty()).then_some(by);
    let primary = owner.map(|by| Published { offer, by });
    with_state(|state| {
        state.primary = primary;
        announce(state, Selection::Primary, owner, &mime_types);
    });
}

fn owns_clipboard(window: HWND) -> bool {
    unsafe { GetClipboardOwner() }.is_ok_and(|owner| owner == window)
}

fn own_offer() -> Option<Published> {
    let (window, published) = with_state(|state| {
        let published = state.published.as_ref().map(|published| Published {
            offer: published.offer.clone(),
            by: published.by,
        });
        (state.window, published)
    })?;
    published.filter(|_| owns_clipboard(window))
}

fn mime_types(selection: Selection) -> Result<Vec<String>> {
    match selection {
        Selection::Primary => Ok(with_state(|state| {
            state.primary.as_ref().map_or_else(Vec::new, |primary| {
                primary.offer.mime_types().map(str::to_owned).collect()
            })
        })
        .unwrap_or_default()),
        Selection::Clipboard => {
            if let Some(own) = own_offer() {
                return Ok(own.offer.mime_types().map(str::to_owned).collect());
            }
            let (window, registered) = with_state(|state| (state.window, state.registered))
                .ok_or(Error::Unavailable("clipboard worker stopped"))?;
            let _opened = Opened::new(window)?;
            Ok(formats::offered(&registered))
        }
    }
}

fn read(selection: Selection, mime: &str, limit: usize) -> Result<Vec<u8>> {
    let local = match selection {
        Selection::Primary => Some(
            with_state(|state| state.primary.as_ref().map(|primary| primary.offer.clone()))
                .flatten()
                .ok_or(Error::Empty)?,
        ),
        Selection::Clipboard => own_offer().map(|own| own.offer),
    };
    if let Some(offer) = local {
        let data = offer
            .data(mime)
            .ok_or_else(|| Error::NoSuchType(mime.to_owned()))?;
        if data.len() > limit {
            return Err(Error::Io(std::io::Error::other(format!(
                "clipboard data of {} bytes exceeds {limit}",
                data.len()
            ))));
        }
        return Ok(data.to_vec());
    }
    let (window, registered) = with_state(|state| (state.window, state.registered))
        .ok_or(Error::Unavailable("clipboard worker stopped"))?;
    let _opened = Opened::new(window)?;
    formats::read(mime, limit, &registered)
}

fn announce_clipboard() {
    let mime_types = mime_types(Selection::Clipboard).unwrap_or_else(|error| {
        tracing::debug!("cannot list clipboard types: {error}");
        Vec::new()
    });
    let owner = own_offer().map(|own| own.by);
    with_state(|state| announce(state, Selection::Clipboard, owner, &mime_types));
}

fn announce(state: &State, selection: Selection, owner: Option<u64>, mime_types: &[String]) {
    for (watching, watcher) in &state.watchers {
        let ours = owner == Some(*watching);
        watcher(&Change::new(selection, ours, mime_types.to_vec()));
    }
}

fn render(format: u32) {
    let Some((offer, registered)) = with_state(|state| {
        let offer = state.published.as_ref()?.offer.clone();
        Some((offer, state.registered))
    })
    .flatten() else {
        return;
    };
    let Some(bytes) = formats::render(&offer, format, &registered) else {
        tracing::debug!("clipboard format {format} has nothing to render");
        return;
    };
    let global = match Global::holding(&bytes) {
        Ok(global) => global,
        Err(error) => {
            tracing::warn!("failed to render clipboard format {format}: {error}");
            return;
        }
    };
    let handle = global.into_handle();
    if let Err(error) = unsafe { SetClipboardData(format, Some(handle)) } {
        tracing::warn!("failed to hand over clipboard format {format}: {error}");
        drop(Global::from_handle(handle));
    }
}

fn render_all() {
    let Some((window, offer, registered)) = with_state(|state| {
        let offer = state.published.as_ref()?.offer.clone();
        Some((state.window, offer, state.registered))
    })
    .flatten() else {
        return;
    };
    let opened = match Opened::new(window) {
        Ok(opened) => opened,
        Err(error) => {
            tracing::warn!("failed to keep clipboard contents: {error}");
            return;
        }
    };
    if !owns_clipboard(window) {
        return;
    }
    for format in formats::publishable(&offer, &registered) {
        render(format);
    }
    drop(opened);
}
