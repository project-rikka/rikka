use rikka_types::Rect;
use rikka_ui::Event;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::UI::Input::Ime::{
    CANDIDATEFORM, CFS_EXCLUDE, CFS_POINT, COMPOSITIONFORM, GCS_COMPSTR, GCS_CURSORPOS,
    GCS_RESULTSTR, HIMC, IACE_DEFAULT, IME_COMPOSITION_STRING, ISC_SHOWUICOMPOSITIONWINDOW,
    ImmAssociateContextEx, ImmGetCompositionStringW, ImmGetContext, ImmReleaseContext,
    ImmSetCandidateWindow, ImmSetCompositionWindow,
};
use windows::Win32::UI::WindowsAndMessaging::DefWindowProcW;

use crate::tasks::WindowId;
use crate::win32::heard;
use crate::win32::input::scale_of;

#[derive(Debug, Default)]
pub(crate) struct Ime {
    enabled: Option<bool>,
    composing: bool,
    placed: Option<RECT>,
}

struct Context {
    window: HWND,
    handle: HIMC,
}

impl Context {
    fn of(window: HWND) -> Option<Context> {
        let handle = unsafe { ImmGetContext(window) };
        (!handle.is_invalid()).then_some(Context { window, handle })
    }

    fn string(&self, kind: IME_COMPOSITION_STRING) -> Option<String> {
        let bytes = unsafe { ImmGetCompositionStringW(self.handle, kind, None, 0) };
        let bytes = usize::try_from(bytes).ok()?;
        let mut units = vec![0u16; bytes / 2];
        let written = unsafe {
            ImmGetCompositionStringW(
                self.handle,
                kind,
                Some(units.as_mut_ptr().cast()),
                u32::try_from(bytes).ok()?,
            )
        };
        let written = usize::try_from(written).ok()?;
        units.truncate(written / 2);
        Some(String::from_utf16_lossy(&units))
    }

    fn cursor(&self) -> Option<usize> {
        let position = unsafe { ImmGetCompositionStringW(self.handle, GCS_CURSORPOS, None, 0) };
        usize::try_from(position).ok()
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        if !unsafe { ImmReleaseContext(self.window, self.handle) }.as_bool() {
            tracing::debug!("failed to release input context");
        }
    }
}

impl Ime {
    pub(crate) fn set_context(
        window: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        let shown = LPARAM(lparam.0 & !(ISC_SHOWUICOMPOSITIONWINDOW as isize));
        unsafe { DefWindowProcW(window, message, wparam, shown) }
    }

    pub(crate) fn started(&mut self, window: HWND) -> LRESULT {
        self.composing = true;
        if let Some(caret) = self.placed {
            place(window, caret);
        }
        LRESULT(0)
    }

    pub(crate) fn composed(&mut self, id: WindowId, window: HWND, lparam: LPARAM) -> LRESULT {
        let Some(context) = Context::of(window) else {
            return LRESULT(0);
        };
        let flags = lparam.0 as u32;
        if flags & GCS_RESULTSTR.0 != 0
            && let Some(result) = context.string(GCS_RESULTSTR)
            && !result.is_empty()
        {
            heard::input(id, Event::Commit(result));
        }
        if flags & GCS_COMPSTR.0 != 0 {
            let text = context.string(GCS_COMPSTR).unwrap_or_default();
            let cursor = context
                .cursor()
                .map(|units| byte_offset(&text, units))
                .map(|at| at..at);
            heard::input(id, Event::Preedit { text, cursor });
        }
        LRESULT(0)
    }

    pub(crate) fn ended(&mut self, id: WindowId) -> LRESULT {
        if std::mem::take(&mut self.composing) {
            heard::input(
                id,
                Event::Preedit {
                    text: String::new(),
                    cursor: None,
                },
            );
        }
        LRESULT(0)
    }

    pub(crate) fn follow(&mut self, window: HWND, caret: Option<Rect>) {
        let enabled = caret.is_some();
        if self.enabled != Some(enabled) {
            self.enabled = Some(enabled);
            let flags = match enabled {
                true => IACE_DEFAULT,
                false => 0,
            };
            if !unsafe { ImmAssociateContextEx(window, HIMC::default(), flags) }.as_bool() {
                tracing::debug!("failed to switch the input method context");
            }
        }
        let Some(caret) = caret else {
            self.placed = None;
            return;
        };
        let scale = scale_of(window);
        let pixels = RECT {
            left: (f64::from(caret.min_x()) * scale).floor() as i32,
            top: (f64::from(caret.min_y()) * scale).floor() as i32,
            right: (f64::from(caret.max_x()) * scale).ceil() as i32,
            bottom: (f64::from(caret.max_y()) * scale).ceil() as i32,
        };
        if self.placed == Some(pixels) {
            return;
        }
        self.placed = Some(pixels);
        if self.composing {
            place(window, pixels);
        }
    }
}

fn place(window: HWND, caret: RECT) {
    let Some(context) = Context::of(window) else {
        return;
    };
    let composition = COMPOSITIONFORM {
        dwStyle: CFS_POINT,
        ptCurrentPos: POINT {
            x: caret.left,
            y: caret.top,
        },
        rcArea: RECT::default(),
    };
    let candidate = CANDIDATEFORM {
        dwIndex: 0,
        dwStyle: CFS_EXCLUDE,
        ptCurrentPos: POINT {
            x: caret.left,
            y: caret.bottom,
        },
        rcArea: caret,
    };
    unsafe {
        if !ImmSetCompositionWindow(context.handle, &composition).as_bool() {
            tracing::debug!("failed to place the composition window");
        }
        if !ImmSetCandidateWindow(context.handle, &candidate).as_bool() {
            tracing::debug!("failed to place the candidate window");
        }
    }
}

fn byte_offset(text: &str, units: usize) -> usize {
    let mut counted = 0;
    for (at, letter) in text.char_indices() {
        if counted >= units {
            return at;
        }
        counted += letter.len_utf16();
    }
    text.len()
}
