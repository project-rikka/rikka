use std::rc::Rc;
use std::sync::OnceLock;

use windows::Win32::Foundation::{
    ERROR_CLASS_ALREADY_EXISTS, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CREATESTRUCTW, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassExW,
    WINDOW_EX_STYLE, WINDOW_STYLE, WM_NCCREATE, WM_NCDESTROY, WNDCLASSEXW,
};
use windows::core::{HRESULT, HSTRING, PCWSTR, w};

use crate::error::Result;
use crate::win32::window::procedure;
use crate::win32::window::shared::{self, Shared};

const CLASS: PCWSTR = w!("RikkaWindow");

#[derive(Clone, Copy, Debug)]
pub(crate) struct WindowClass {
    instance: HINSTANCE,
}

impl WindowClass {
    pub(crate) fn register() -> Result<WindowClass> {
        static REGISTERED: OnceLock<Result<(), HRESULT>> = OnceLock::new();
        let instance = HINSTANCE(unsafe { GetModuleHandleW(None) }?.0);
        let registered = REGISTERED.get_or_init(|| {
            let class = WNDCLASSEXW {
                cbSize: size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(window_procedure),
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
        match registered {
            Ok(()) => Ok(WindowClass { instance }),
            Err(code) => Err(windows::core::Error::from_hresult(*code).into()),
        }
    }
}

#[derive(Debug)]
pub(crate) struct NativeWindow {
    window: HWND,
    shared: Rc<Shared>,
}

pub(crate) struct Creation<'a> {
    pub(crate) title: &'a str,
    pub(crate) style: WINDOW_STYLE,
    pub(crate) extended: WINDOW_EX_STYLE,
    pub(crate) owner: Option<HWND>,
}

impl NativeWindow {
    pub(crate) fn create(
        class: WindowClass,
        shared: Rc<Shared>,
        creation: &Creation<'_>,
    ) -> Result<NativeWindow> {
        let window = unsafe {
            CreateWindowExW(
                creation.extended,
                CLASS,
                &HSTRING::from(creation.title),
                creation.style,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                creation.owner,
                None,
                Some(class.instance),
                Some(Rc::as_ptr(&shared).cast()),
            )
        }?;
        Ok(NativeWindow { window, shared })
    }

    pub(crate) fn hwnd(&self) -> HWND {
        self.window
    }

    pub(crate) fn shared(&self) -> &Rc<Shared> {
        &self.shared
    }
}

impl Drop for NativeWindow {
    fn drop(&mut self) {
        if let Err(error) = unsafe { DestroyWindow(self.window) } {
            tracing::error!("failed to destroy window: {error}");
            shared::detach(self.window);
        }
    }
}

unsafe extern "system" fn window_procedure(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        let creation = unsafe { &*(lparam.0 as *const CREATESTRUCTW) };
        shared::attach(window, creation.lpCreateParams.cast_const().cast());
        return unsafe { DefWindowProcW(window, message, wparam, lparam) };
    }
    let attached = shared::attached(window);
    if attached.is_null() {
        return unsafe { DefWindowProcW(window, message, wparam, lparam) };
    }
    let shared = unsafe {
        Rc::increment_strong_count(attached);
        Rc::from_raw(attached)
    };
    let answered = procedure::handle(&shared, window, message, wparam, lparam);
    if message == WM_NCDESTROY {
        shared::detach(window);
    }
    answered.unwrap_or_else(|| unsafe { DefWindowProcW(window, message, wparam, lparam) })
}
