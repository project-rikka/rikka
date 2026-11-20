use std::time::{Duration, Instant};

use rikka_input::win32::{DEFAULT_WHEEL_LINES, wheel_offset};
use rikka_input::{ScrollPhase, ScrollSource};
use rikka_types::{Modifiers, Point, PointerButton, ScrollDelta};
use rikka_ui::Event;
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetCapture, ReleaseCapture, SetCapture, TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT,
    TrackMouseEvent,
};
use windows::Win32::UI::Input::{
    GetCurrentInputMessageSource, IMDT_TOUCHPAD, INPUT_MESSAGE_SOURCE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    SPI_GETWHEELSCROLLCHARS, SPI_GETWHEELSCROLLLINES, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    SystemParametersInfoW,
};

use crate::tasks::WindowId;
use crate::win32::heard;
use crate::win32::input::logical;

const CONTINUOUS_QUIET: Duration = Duration::from_millis(120);

#[derive(Debug, Default)]
pub(crate) struct Mouse {
    tracking: Option<Area>,
    held: Vec<PointerButton>,
    position: Point,
    continuous: Option<Continuous>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Area {
    Client,
    Frame,
}

#[derive(Clone, Copy, Debug)]
struct Continuous {
    last: Instant,
    modifiers: Modifiers,
}

impl Mouse {
    pub(crate) fn moved(&mut self, id: WindowId, window: HWND, point: Point) {
        self.track(window, Area::Client);
        self.position = point;
        heard::input(id, Event::PointerMoved(point));
    }

    pub(crate) fn moved_over_frame(&mut self, id: WindowId, window: HWND, point: Point) {
        self.track(window, Area::Frame);
        self.position = point;
        heard::input(id, Event::PointerMoved(point));
    }

    pub(crate) fn left(&mut self, id: WindowId, area_is_frame: bool) {
        let area = match area_is_frame {
            true => Area::Frame,
            false => Area::Client,
        };
        if self.tracking != Some(area) {
            return;
        }
        self.tracking = None;
        if self.held.is_empty() {
            heard::input(id, Event::PointerLeft);
        }
    }

    pub(crate) fn button(
        &mut self,
        id: WindowId,
        window: HWND,
        point: Point,
        button: PointerButton,
        pressed: bool,
        modifiers: Modifiers,
    ) {
        self.position = point;
        match pressed {
            true => {
                if self.held.is_empty() && unsafe { GetCapture() } != window {
                    unsafe { SetCapture(window) };
                }
                if !self.held.contains(&button) {
                    self.held.push(button);
                }
                heard::input(
                    id,
                    Event::PointerPressed {
                        position: point,
                        button,
                        modifiers,
                    },
                );
            }
            false => {
                let was_held = self.held.contains(&button);
                self.held.retain(|held| *held != button);
                if self.held.is_empty()
                    && unsafe { GetCapture() } == window
                    && let Err(error) = unsafe { ReleaseCapture() }
                {
                    tracing::debug!("failed to release pointer capture: {error}");
                }
                if was_held {
                    heard::input(
                        id,
                        Event::PointerReleased {
                            position: point,
                            button,
                            modifiers,
                        },
                    );
                }
            }
        }
    }

    pub(crate) fn capture_lost(&mut self) {
        self.held.clear();
    }

    pub(crate) fn hovers_client(&self) -> bool {
        self.tracking == Some(Area::Client)
    }

    pub(crate) fn wheel(
        &mut self,
        id: WindowId,
        window: HWND,
        screen: (i32, i32),
        delta: i32,
        horizontal: bool,
        modifiers: Modifiers,
    ) {
        let mut at = POINT {
            x: screen.0,
            y: screen.1,
        };
        if !unsafe { ScreenToClient(window, &mut at) }.as_bool() {
            return;
        }
        let position = logical(window, at.x, at.y);
        self.position = position;
        let lines = wheel_lines(horizontal);
        let delta = ScrollDelta::from_offset(wheel_offset(delta, lines, horizontal));
        let source = match from_touchpad() {
            true => {
                let now = Instant::now();
                let phase = match self.continuous.replace(Continuous {
                    last: now,
                    modifiers,
                }) {
                    Some(_) => ScrollPhase::Moved,
                    None => ScrollPhase::Began,
                };
                ScrollSource::Continuous(phase)
            }
            false => ScrollSource::Wheel,
        };
        heard::input(
            id,
            Event::Scroll {
                position,
                delta,
                modifiers,
                source,
            },
        );
    }

    pub(crate) fn scroll_due(&self) -> Option<Instant> {
        self.continuous
            .map(|continuous| continuous.last + CONTINUOUS_QUIET)
    }

    pub(crate) fn settle_scroll(&mut self, id: WindowId, now: Instant) {
        let Some(continuous) = self
            .continuous
            .filter(|continuous| now >= continuous.last + CONTINUOUS_QUIET)
        else {
            return;
        };
        self.continuous = None;
        heard::input(
            id,
            Event::Scroll {
                position: self.position,
                delta: ScrollDelta::ZERO,
                modifiers: continuous.modifiers,
                source: ScrollSource::Continuous(ScrollPhase::Ended),
            },
        );
    }

    fn track(&mut self, window: HWND, area: Area) {
        if self.tracking == Some(area) {
            return;
        }
        let flags = match area {
            Area::Client => TME_LEAVE,
            Area::Frame => TME_LEAVE | TME_NONCLIENT,
        };
        let mut request = TRACKMOUSEEVENT {
            cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
            dwFlags: flags,
            hwndTrack: window,
            dwHoverTime: 0,
        };
        match unsafe { TrackMouseEvent(&mut request) } {
            Ok(()) => self.tracking = Some(area),
            Err(error) => tracing::debug!("failed to track pointer leave: {error}"),
        }
    }
}

fn wheel_lines(horizontal: bool) -> u32 {
    let action = match horizontal {
        true => SPI_GETWHEELSCROLLCHARS,
        false => SPI_GETWHEELSCROLLLINES,
    };
    let mut lines = DEFAULT_WHEEL_LINES;
    match unsafe {
        SystemParametersInfoW(
            action,
            0,
            Some((&raw mut lines).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    } {
        Ok(()) => lines,
        Err(error) => {
            tracing::debug!("failed to read wheel scroll setting: {error}");
            DEFAULT_WHEEL_LINES
        }
    }
}

fn from_touchpad() -> bool {
    let mut source = INPUT_MESSAGE_SOURCE::default();
    unsafe { GetCurrentInputMessageSource(&mut source) }.is_ok()
        && source.deviceType == IMDT_TOUCHPAD
}
