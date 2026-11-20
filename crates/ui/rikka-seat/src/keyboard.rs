use std::os::fd::OwnedFd;
use std::time::{Duration, Instant};

use rikka_input::DeviceEvent;
use rikka_input::xkb::{self, KeyRepeat, Keystroke, SessionKeyboard};
use rikka_types::{KeyboardState, Modifiers};
use rikka_wayland_client::wl_keyboard::{self, KeyState, KeymapFormat, WlKeyboard};
use rikka_wayland_client::wl_surface::WlSurface;

use crate::seat_event::SeatEvent;

const RELEASE_SINCE_VERSION: u32 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Rate {
    delay: Duration,
    gap: Duration,
}

#[derive(Debug)]
struct Repeat {
    keystroke: Keystroke,
    ticking: Option<(Instant, Duration)>,
}

#[derive(Debug)]
pub(crate) struct Keyboard {
    pub(crate) keyboard: WlKeyboard,
    xkb: xkb::Keyboard,
    focus: Option<WlSurface>,
    rate: Option<Rate>,
    repeat: Option<Repeat>,
    layout_changed: bool,
}

impl Keyboard {
    pub(crate) fn new(keyboard: WlKeyboard) -> Keyboard {
        Keyboard {
            keyboard,
            xkb: xkb::Keyboard::new(),
            focus: None,
            rate: None,
            repeat: None,
            layout_changed: true,
        }
    }

    pub(crate) fn release(self) {
        if self.keyboard.version() >= RELEASE_SINCE_VERSION {
            self.keyboard.release();
        }
    }

    pub(crate) fn keyboard_state(&self) -> KeyboardState {
        KeyboardState {
            caps_lock: self.xkb.is_caps_locked(),
            layout: self.xkb.layout(),
            layouts: self.xkb.layout_count(),
        }
    }

    pub(crate) fn take_layout_changed(&mut self) -> bool {
        std::mem::take(&mut self.layout_changed)
    }

    pub(crate) fn is_focused(&self) -> bool {
        self.focus.is_some()
    }

    pub(crate) fn repeat_due(&self) -> Option<Instant> {
        self.repeat.as_ref()?.ticking.map(|(due, _)| due)
    }

    pub(crate) fn repeat(&mut self, now: Instant, modifiers: Modifiers, out: &mut Vec<SeatEvent>) {
        let Some(repeat) = self.repeat.as_mut() else {
            return;
        };
        let Some((due, gap)) = repeat.ticking else {
            return;
        };
        if due > now {
            return;
        }
        let Some(surface) = self.focus.clone().filter(WlSurface::alive) else {
            repeat.ticking = None;
            return;
        };
        repeat.ticking = Some((now + gap, gap));
        out.push(SeatEvent::Input {
            surface,
            event: key_event(&repeat.keystroke, true, modifiers),
        });
    }

    pub(crate) fn handle(
        &mut self,
        event: wl_keyboard::Event,
        modifiers: &mut Modifiers,
        now: Instant,
        out: &mut Vec<SeatEvent>,
    ) {
        self.layout_changed |= matches!(
            event,
            wl_keyboard::Event::Keymap { .. }
                | wl_keyboard::Event::Enter { .. }
                | wl_keyboard::Event::Leave { .. }
                | wl_keyboard::Event::Modifiers { .. }
        );
        match event {
            wl_keyboard::Event::Keymap { format, fd, size } => self.load(format, fd, size),
            wl_keyboard::Event::Enter { surface, .. } => {
                self.focus = Some(surface.clone());
                out.push(SeatEvent::Keyboard {
                    surface,
                    entered: true,
                });
            }
            wl_keyboard::Event::Leave { surface, .. } => {
                self.repeat = None;
                self.focus = None;
                out.push(SeatEvent::Keyboard {
                    surface,
                    entered: false,
                });
            }
            wl_keyboard::Event::Key { key, state, .. } => {
                if let Some((keystroke, pressed)) = self.key(key, state, now)
                    && let Some(surface) = self.focus.clone()
                {
                    out.push(SeatEvent::Input {
                        surface,
                        event: key_event(&keystroke, pressed, *modifiers),
                    });
                }
            }
            wl_keyboard::Event::Modifiers {
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
                ..
            } => {
                let Some(modifiers_state) =
                    self.xkb
                        .set_modifiers(mods_depressed, mods_latched, mods_locked, group)
                else {
                    return;
                };
                if let Some(repeat) = self.repeat.as_mut() {
                    self.xkb.refresh_text(&mut repeat.keystroke);
                }
                *modifiers = modifiers_state;
            }
            wl_keyboard::Event::RepeatInfo { rate, delay } => {
                SessionKeyboard::publish_repeat(KeyRepeat { rate, delay });
                self.rate = rate_of(rate, delay);
            }
            _ => {}
        }
    }

    fn load(
        &mut self,
        format: Result<KeymapFormat, rikka_wayland_client::UnknownValue>,
        fd: OwnedFd,
        size: u32,
    ) {
        match format {
            Ok(KeymapFormat::XkbV1) => {
                let Ok(size) = usize::try_from(size) else {
                    tracing::error!("keymap size exceeds address space");
                    return;
                };
                match unsafe { self.xkb.load(fd, size) } {
                    Ok(()) => {
                        if let Some(text) = self.xkb.keymap_text() {
                            SessionKeyboard::publish_keymap(text);
                        }
                    }
                    Err(error) => tracing::error!("{error}"),
                }
            }
            Ok(KeymapFormat::NoKeymap) => {
                tracing::warn!("xkb failed to compile keymap");
            }
            other => tracing::warn!("unsupported keymap format: {other:?}"),
        }
    }

    fn key(
        &mut self,
        code: u32,
        state: Result<KeyState, rikka_wayland_client::UnknownValue>,
        now: Instant,
    ) -> Option<(Keystroke, bool)> {
        match state {
            Ok(KeyState::Pressed) => {
                let keystroke = self.xkb.press(code)?;
                if self.xkb.key_repeats(code) {
                    self.start_repeat(&keystroke, now);
                }
                Some((keystroke, true))
            }
            Ok(KeyState::Released) => {
                let keystroke = self.xkb.keystroke(code)?;
                if self
                    .repeat
                    .as_ref()
                    .is_some_and(|repeat| repeat.keystroke.is_key(code))
                {
                    self.repeat = None;
                }
                Some((keystroke, false))
            }
            Ok(KeyState::Repeated) => self.xkb.keystroke(code).map(|keystroke| (keystroke, true)),
            unknown => {
                tracing::warn!("{}: unknown key state: {unknown:?}", self.keyboard.id());
                None
            }
        }
    }

    fn start_repeat(&mut self, keystroke: &Keystroke, now: Instant) {
        self.repeat = Some(Repeat {
            keystroke: keystroke.clone(),
            ticking: self.rate.map(|rate| (now + rate.delay, rate.gap)),
        });
    }
}

fn key_event(keystroke: &Keystroke, pressed: bool, modifiers: Modifiers) -> DeviceEvent {
    let (key, code, location) = (keystroke.key(), keystroke.code(), keystroke.location());
    match pressed {
        true => DeviceEvent::KeyPressed {
            key,
            code,
            location,
            modifiers,
        },
        false => DeviceEvent::KeyReleased {
            key,
            code,
            location,
            modifiers,
        },
    }
}

fn rate_of(rate: i32, delay: i32) -> Option<Rate> {
    let per_second = u64::try_from(rate).ok().filter(|rate| *rate > 0)?;
    Some(Rate {
        delay: Duration::from_millis(u64::try_from(delay).unwrap_or(0)),
        gap: Duration::from_micros(1_000_000 / per_second),
    })
}
