use rikka_input::win32::{key_code, location, named_key};
use rikka_types::{Key, KeyCode, KeyLocation};
use rikka_ui::Event;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyboardLayout, GetKeyboardState, MAPVK_VK_TO_VSC_EX, MapVirtualKeyW, ToUnicodeEx,
    VK_CONTROL, VK_F4, VK_LCONTROL, VK_LMENU, VK_MENU, VK_PACKET, VK_PROCESSKEY, VK_RCONTROL,
    VK_RMENU, VK_SPACE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetMessageTime, MSG, PM_NOREMOVE, PM_REMOVE, PeekMessageW, WM_CHAR, WM_DEADCHAR, WM_KEYDOWN,
    WM_KEYFIRST, WM_KEYLAST, WM_KEYUP, WM_SYSCHAR, WM_SYSDEADCHAR, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

use crate::tasks::WindowId;
use crate::win32::heard;
use crate::win32::input::{grab, modifiers};

const EXTENDED: isize = 1 << 24;
const KEEP_KERNEL_STATE: u32 = 1 << 2;
const TEXT_LENGTH: usize = 8;

#[derive(Debug, Default)]
pub(crate) struct Keyboard {
    fake_control: bool,
    altgr: bool,
    surrogate: Option<u16>,
}

struct Stroke {
    virtual_key: u16,
    scan: u16,
    extended: bool,
}

impl Stroke {
    fn of(wparam: WPARAM, lparam: LPARAM) -> Stroke {
        let virtual_key = wparam.0 as u16;
        let extended = lparam.0 & EXTENDED != 0;
        let scan = match ((lparam.0 >> 16) & 0xFF) as u16 {
            0 => {
                let mapped = unsafe { MapVirtualKeyW(u32::from(virtual_key), MAPVK_VK_TO_VSC_EX) };
                (mapped & 0xFF) as u16
            }
            scan => scan,
        };
        Stroke {
            virtual_key,
            scan,
            extended,
        }
    }

    fn code(&self) -> KeyCode {
        key_code(self.scan, self.extended)
    }

    fn location(&self) -> KeyLocation {
        location(self.scan, self.extended)
    }
}

impl Keyboard {
    pub(crate) fn altgr(&self) -> bool {
        self.altgr
    }

    pub(crate) fn key(
        &mut self,
        id: WindowId,
        window: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Option<LRESULT> {
        let stroke = Stroke::of(wparam, lparam);
        let pressed = matches!(message, WM_KEYDOWN | WM_SYSKEYDOWN);
        if stroke.virtual_key == VK_PROCESSKEY.0 || stroke.virtual_key == VK_PACKET.0 {
            return None;
        }
        if self.swallows_fake_control(window, &stroke, pressed) {
            return Some(LRESULT(0));
        }
        if stroke.virtual_key == VK_MENU.0 && stroke.extended && !pressed {
            self.altgr = false;
        }
        let modifiers = modifiers(self.altgr);
        if message == WM_SYSKEYDOWN
            && !grab::grabs(window)
            && passes_to_system(&stroke, modifiers.alt)
        {
            return None;
        }
        let event = match pressed {
            true => {
                let text = typed(window);
                let key = named_key(stroke.virtual_key, stroke.scan, stroke.extended)
                    .or_else(|| text.as_deref().and_then(single_character))
                    .unwrap_or_else(|| self.layout_key(&stroke));
                Event::KeyPressed {
                    key,
                    code: stroke.code(),
                    location: stroke.location(),
                    modifiers,
                }
            }
            false => Event::KeyReleased {
                key: named_key(stroke.virtual_key, stroke.scan, stroke.extended)
                    .unwrap_or_else(|| self.layout_key(&stroke)),
                code: stroke.code(),
                location: stroke.location(),
                modifiers,
            },
        };
        heard::input(id, event);
        Some(LRESULT(0))
    }

    pub(crate) fn character(&mut self, id: WindowId, wparam: WPARAM) -> LRESULT {
        let unit = wparam.0 as u16;
        let units: Vec<u16> = match (self.surrogate.take(), unit) {
            (_, 0xD800..=0xDBFF) => {
                self.surrogate = Some(unit);
                return LRESULT(0);
            }
            (Some(high), 0xDC00..=0xDFFF) => vec![high, unit],
            (_, unit) => vec![unit],
        };
        let text = String::from_utf16_lossy(&units);
        if text.chars().any(|letter| !letter.is_control()) {
            heard::input(id, Event::Commit(text));
        }
        LRESULT(0)
    }

    fn swallows_fake_control(&mut self, window: HWND, stroke: &Stroke, pressed: bool) -> bool {
        if stroke.virtual_key != VK_CONTROL.0 || stroke.extended {
            return false;
        }
        if !pressed {
            return std::mem::take(&mut self.fake_control);
        }
        let mut next = MSG::default();
        let peeked = unsafe {
            PeekMessageW(
                &mut next,
                Some(window),
                WM_KEYFIRST,
                WM_KEYLAST,
                PM_NOREMOVE,
            )
        }
        .as_bool();
        let altgr = peeked
            && matches!(next.message, WM_KEYDOWN | WM_SYSKEYDOWN)
            && next.wParam.0 as u16 == VK_MENU.0
            && next.lParam.0 & EXTENDED != 0
            && next.time == unsafe { GetMessageTime() } as u32;
        if altgr {
            self.fake_control = true;
            self.altgr = true;
        }
        altgr
    }

    fn layout_key(&self, stroke: &Stroke) -> Key {
        let mut state = [0u8; 256];
        if let Err(error) = unsafe { GetKeyboardState(&mut state) } {
            tracing::debug!("failed to read keyboard state: {error}");
            return Key::Unidentified;
        }
        if !self.altgr {
            for key in [
                VK_CONTROL,
                VK_LCONTROL,
                VK_RCONTROL,
                VK_MENU,
                VK_LMENU,
                VK_RMENU,
            ] {
                state[usize::from(key.0)] = 0;
            }
        }
        let mut buffer = [0u16; TEXT_LENGTH];
        let layout = unsafe { GetKeyboardLayout(0) };
        let written = unsafe {
            ToUnicodeEx(
                u32::from(stroke.virtual_key),
                u32::from(stroke.scan),
                &state,
                &mut buffer,
                KEEP_KERNEL_STATE,
                Some(layout),
            )
        };
        let Ok(written) = usize::try_from(written) else {
            return Key::Unidentified;
        };
        single_character(&String::from_utf16_lossy(
            &buffer[..written.min(TEXT_LENGTH)],
        ))
        .unwrap_or(Key::Unidentified)
    }
}

fn passes_to_system(stroke: &Stroke, alt: bool) -> bool {
    alt && (stroke.virtual_key == VK_F4.0 || stroke.virtual_key == VK_SPACE.0)
}

fn typed(window: HWND) -> Option<String> {
    let mut units = Vec::new();
    loop {
        let mut next = MSG::default();
        let peeked = unsafe {
            PeekMessageW(
                &mut next,
                Some(window),
                WM_KEYFIRST,
                WM_KEYLAST,
                PM_NOREMOVE,
            )
        }
        .as_bool();
        if !peeked
            || !matches!(
                next.message,
                WM_CHAR | WM_SYSCHAR | WM_DEADCHAR | WM_SYSDEADCHAR
            )
        {
            break;
        }
        let mut taken = MSG::default();
        let removed = unsafe {
            PeekMessageW(
                &mut taken,
                Some(window),
                next.message,
                next.message,
                PM_REMOVE,
            )
        }
        .as_bool();
        if !removed {
            break;
        }
        if matches!(taken.message, WM_CHAR | WM_SYSCHAR) {
            units.push(taken.wParam.0 as u16);
        }
    }
    (!units.is_empty()).then(|| String::from_utf16_lossy(&units))
}

fn single_character(text: &str) -> Option<Key> {
    let mut letters = text.chars();
    let letter = letters.next()?;
    (letters.next().is_none() && !letter.is_control()).then_some(Key::Char(letter))
}

pub(crate) fn is_key_message(message: u32) -> bool {
    matches!(message, WM_KEYDOWN | WM_KEYUP | WM_SYSKEYDOWN | WM_SYSKEYUP)
}
