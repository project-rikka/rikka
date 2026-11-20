use std::collections::HashMap;
use std::time::Duration;

use rikka_types::{
    Modifiers, Point, PointerButton, Stylus, StylusSample, Tilt, Tool, ToolCapabilities, ToolKind,
};
use rikka_ui::Event;
use windows::Win32::Foundation::{HWND, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::UI::Input::Pointer::{
    GetPointerInfo, GetPointerPenInfo, GetPointerType, POINTER_FLAG_INCONTACT,
    POINTER_FLAG_PRIMARY, POINTER_INFO, POINTER_PEN_INFO,
};
use windows::Win32::UI::WindowsAndMessaging::{
    PEN_FLAG_BARREL, PEN_FLAG_ERASER, PEN_FLAG_INVERTED, PEN_MASK_PRESSURE, PEN_MASK_ROTATION,
    PEN_MASK_TILT_X, PEN_MASK_TILT_Y, POINTER_INPUT_TYPE, PT_PEN, PT_TOUCH,
    WM_POINTERCAPTURECHANGED, WM_POINTERDOWN, WM_POINTERLEAVE, WM_POINTERUP,
};

use crate::tasks::WindowId;
use crate::win32::heard::{self, Heard};
use crate::win32::input::{logical, modifiers};

const PRESSURE_RANGE: f32 = 1024.0;

#[derive(Debug, Default)]
pub(crate) struct Pointers {
    pens: HashMap<u32, Pen>,
    touch: Option<u32>,
}

#[derive(Clone, Copy, Debug, Default)]
struct Pen {
    contact: bool,
    barrel: bool,
}

impl Pointers {
    pub(crate) fn handle(
        &mut self,
        id: WindowId,
        window: HWND,
        message: u32,
        wparam: WPARAM,
    ) -> Option<LRESULT> {
        let pointer = (wparam.0 & 0xFFFF) as u32;
        let mut kind = POINTER_INPUT_TYPE::default();
        unsafe { GetPointerType(pointer, &mut kind) }.ok()?;
        match kind {
            PT_PEN => self.pen(id, window, message, pointer),
            PT_TOUCH => self.touch(id, window, message, pointer),
            _ => None,
        }
    }

    fn pen(&mut self, id: WindowId, window: HWND, message: u32, pointer: u32) -> Option<LRESULT> {
        let mut info = POINTER_PEN_INFO::default();
        if let Err(error) = unsafe { GetPointerPenInfo(pointer, &mut info) } {
            tracing::debug!("failed to read pen input: {error}");
            return None;
        }
        let position = client(window, &info.pointerInfo)?;
        let stylus = stylus_of(&info, position);
        let modifiers = modifiers(false);
        let send = |event: Event| {
            heard::hear(
                id,
                Heard::Input {
                    event,
                    stylus: Some(stylus.clone()),
                },
            );
        };
        let state = self.pens.entry(pointer).or_default();
        let contact = info.pointerInfo.pointerFlags.0 & POINTER_FLAG_INCONTACT.0 != 0;
        let barrel = info.penFlags & PEN_FLAG_BARREL != 0;
        if matches!(message, WM_POINTERLEAVE | WM_POINTERCAPTURECHANGED) {
            let held = *state;
            self.pens.remove(&pointer);
            release_held(held, position, modifiers, &send);
            send(Event::PointerLeft);
            return Some(LRESULT(0));
        }
        send(Event::PointerMoved(position));
        if barrel != state.barrel {
            state.barrel = barrel;
            send(button_event(
                PointerButton::Secondary,
                barrel,
                position,
                modifiers,
            ));
        }
        let touching = match message {
            WM_POINTERDOWN => true,
            WM_POINTERUP => false,
            _ => contact,
        };
        if touching != state.contact {
            state.contact = touching;
            send(button_event(
                PointerButton::Primary,
                touching,
                position,
                modifiers,
            ));
        }
        Some(LRESULT(0))
    }

    fn touch(&mut self, id: WindowId, window: HWND, message: u32, pointer: u32) -> Option<LRESULT> {
        let mut info = POINTER_INFO::default();
        if let Err(error) = unsafe { GetPointerInfo(pointer, &mut info) } {
            tracing::debug!("failed to read touch input: {error}");
            return None;
        }
        let primary = info.pointerFlags.0 & POINTER_FLAG_PRIMARY.0 != 0;
        let owner = match self.touch {
            Some(finger) => finger == pointer,
            None => primary && message == WM_POINTERDOWN,
        };
        if !owner {
            return Some(LRESULT(0));
        }
        let position = client(window, &info)?;
        let modifiers = modifiers(false);
        match message {
            WM_POINTERDOWN => {
                self.touch = Some(pointer);
                heard::input(id, Event::PointerMoved(position));
                heard::input(
                    id,
                    button_event(PointerButton::Primary, true, position, modifiers),
                );
            }
            WM_POINTERUP | WM_POINTERCAPTURECHANGED => {
                self.touch = None;
                heard::input(
                    id,
                    button_event(PointerButton::Primary, false, position, modifiers),
                );
                heard::input(id, Event::PointerLeft);
            }
            _ => heard::input(id, Event::PointerMoved(position)),
        }
        Some(LRESULT(0))
    }
}

fn release_held(held: Pen, position: Point, modifiers: Modifiers, send: &impl Fn(Event)) {
    if held.contact {
        send(button_event(
            PointerButton::Primary,
            false,
            position,
            modifiers,
        ));
    }
    if held.barrel {
        send(button_event(
            PointerButton::Secondary,
            false,
            position,
            modifiers,
        ));
    }
}

fn button_event(
    button: PointerButton,
    pressed: bool,
    position: Point,
    modifiers: Modifiers,
) -> Event {
    match pressed {
        true => Event::PointerPressed {
            position,
            button,
            modifiers,
        },
        false => Event::PointerReleased {
            position,
            button,
            modifiers,
        },
    }
}

fn client(window: HWND, info: &POINTER_INFO) -> Option<Point> {
    let mut at = POINT {
        x: info.ptPixelLocation.x,
        y: info.ptPixelLocation.y,
    };
    unsafe { ScreenToClient(window, &mut at) }
        .as_bool()
        .then(|| logical(window, at.x, at.y))
}

fn stylus_of(info: &POINTER_PEN_INFO, position: Point) -> Stylus {
    let erases = info.penFlags & (PEN_FLAG_ERASER | PEN_FLAG_INVERTED) != 0;
    let has = |mask: u32| info.penMask & mask != 0;
    let tool = Tool {
        kind: match erases {
            true => ToolKind::Eraser,
            false => ToolKind::Pen,
        },
        serial: info.pointerInfo.sourceDevice.0 as u64,
        hardware_id: 0,
        capabilities: ToolCapabilities {
            pressure: has(PEN_MASK_PRESSURE),
            tilt: has(PEN_MASK_TILT_X) || has(PEN_MASK_TILT_Y),
            distance: false,
            rotation: has(PEN_MASK_ROTATION),
            slider: false,
            wheel: false,
        },
    };
    let sample = StylusSample {
        position,
        pressure: has(PEN_MASK_PRESSURE).then(|| info.pressure as f32 / PRESSURE_RANGE),
        distance: None,
        tilt: (has(PEN_MASK_TILT_X) || has(PEN_MASK_TILT_Y))
            .then(|| Tilt::new(info.tiltX as f32, info.tiltY as f32)),
        rotation: has(PEN_MASK_ROTATION).then_some(info.rotation as f32),
        slider: None,
        contact: info.pointerInfo.pointerFlags.0 & POINTER_FLAG_INCONTACT.0 != 0,
        time: Duration::from_millis(u64::from(info.pointerInfo.dwTime)),
    };
    Stylus::new(tool, sample)
}
