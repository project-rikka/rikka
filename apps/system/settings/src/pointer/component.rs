use std::sync::Arc;

use rikka_app::prelude::*;
use rikka_settings::session::pointer::{
    CURSOR_SIZE, CURSOR_THEME, DOMAIN, MOUSE_ACCELERATION, MOUSE_ENABLED, MOUSE_LEFT_HANDED,
    MOUSE_MIDDLE_EMULATION, MOUSE_NATURAL_SCROLL, MOUSE_ROTATION, MOUSE_SCROLL_BUTTON,
    MOUSE_SCROLL_BUTTON_LOCK, MOUSE_SCROLL_FACTOR, MOUSE_SCROLL_METHOD, MOUSE_SPEED,
    TOUCHPAD_ACCELERATION, TOUCHPAD_CLICK_METHOD, TOUCHPAD_DISABLE_WHILE_POINTING,
    TOUCHPAD_DISABLE_WHILE_TYPING, TOUCHPAD_LEFT_HANDED, TOUCHPAD_MIDDLE_EMULATION,
    TOUCHPAD_NATURAL_SCROLL, TOUCHPAD_SCROLL_FACTOR, TOUCHPAD_SCROLL_METHOD, TOUCHPAD_SPEED,
    TOUCHPAD_TAP, TOUCHPAD_TAP_BUTTONS, TOUCHPAD_TAP_DRAG, TOUCHPAD_TAP_DRAG_LOCK, TOUCHPAD_USED,
};
use rikka_widgets::column;

use crate::amount::amount;
use crate::controls::{Choices, choice, flag};
use crate::domain::{Domain, domain_page};
use crate::notice::domain_notice;
use crate::page::Page;
use crate::pending::Pending;
use crate::picker::{default_picker, picker};
use crate::pointer::themes::{self, CursorTheme};
use crate::session::Session;

const ACCELERATION: Choices = &[
    ("adaptive", "settings.pointer.accel.adaptive"),
    ("flat", "settings.pointer.accel.flat"),
];
const MOUSE_SCROLLING: Choices = &[
    ("wheel", "settings.pointer.mouse-scrolling.wheel"),
    ("button", "settings.pointer.mouse-scrolling.button"),
];
const TOUCHPAD_USE: Choices = &[
    ("always", "settings.pointer.touchpad-use.always"),
    ("never", "settings.pointer.touchpad-use.never"),
    ("no-mouse", "settings.pointer.touchpad-use.no-mouse"),
];
const TAP_BUTTONS: Choices = &[
    (
        "left-right-middle",
        "settings.pointer.taps.left-right-middle",
    ),
    (
        "left-middle-right",
        "settings.pointer.taps.left-middle-right",
    ),
];
const CLICKING: Choices = &[
    ("areas", "settings.pointer.clicking.areas"),
    ("fingers", "settings.pointer.clicking.fingers"),
];
const TOUCHPAD_SCROLLING: Choices = &[
    (
        "two-finger",
        "settings.pointer.touchpad-scrolling.two-finger",
    ),
    ("edge", "settings.pointer.touchpad-scrolling.edge"),
    ("none", "settings.pointer.touchpad-scrolling.none"),
];

const SCROLL_BUTTONS: [(i64, &str); 7] = [
    (273, "settings.pointer.button.right"),
    (274, "settings.pointer.button.middle"),
    (275, "settings.pointer.button.side"),
    (276, "settings.pointer.button.extra"),
    (277, "settings.pointer.button.forward"),
    (278, "settings.pointer.button.back"),
    (279, "settings.pointer.button.task"),
];

const ROTATIONS: [i64; 4] = [0, 90, 180, 270];
const CURSOR_SIZES: [i64; 6] = [16, 24, 32, 48, 64, 96];

pub(crate) struct Pointer {
    domain: Domain,
    themes: Arc<[CursorTheme]>,
}

impl Page for Pointer {
    fn opened(&mut self, _: &Pending, session: &Session) {
        self.domain.open::<Self>(session);
        if self.themes.is_empty() {
            spawn_blocking(
                themes::installed,
                |pointer: &mut Pointer, installed: Vec<CursorTheme>| {
                    pointer.themes = installed.into();
                },
            );
        }
    }
}

domain_page!(Pointer);

impl Pointer {
    pub(crate) fn new() -> Self {
        Self {
            domain: Domain::new(DOMAIN),
            themes: Arc::from(Vec::new()),
        }
    }

    fn on(&self, key: &str) -> bool {
        self.domain.get(key).unwrap_or(false)
    }

    fn count(&self, key: &str) -> i64 {
        self.domain.get(key).unwrap_or_default()
    }
}

pub(crate) fn view(pointer: &Pointer) -> impl View {
    column! {
        domain_notice(
            &pointer.domain,
            "settings.pointer.unavailable",
            "settings.pointer.refused",
        );
        section(tr!("settings.pointer.mouse"), mouse(pointer));
        section(tr!("settings.pointer.touchpad"), touchpad(pointer));
        section(tr!("settings.pointer.cursor"), cursor(pointer));
    }
    .gap(Space::ROOMY)
}

fn mouse(pointer: &Pointer) -> impl View {
    column! {
        flag(pointer, tr!("settings.pointer.mouse-enabled"), MOUSE_ENABLED);
        speed(pointer, tr!("settings.pointer.speed"), MOUSE_SPEED);
        choice(
            pointer,
            tr!("settings.pointer.acceleration"),
            MOUSE_ACCELERATION,
            ACCELERATION,
        );
        flag(pointer, tr!("settings.pointer.natural-scroll"), MOUSE_NATURAL_SCROLL);
        flag(pointer, tr!("settings.pointer.left-handed"), MOUSE_LEFT_HANDED);
        flag(pointer, tr!("settings.pointer.middle-emulation"), MOUSE_MIDDLE_EMULATION);
        factor(pointer, tr!("settings.pointer.scroll-factor"), MOUSE_SCROLL_FACTOR);
        choice(
            pointer,
            tr!("settings.pointer.scroll-method"),
            MOUSE_SCROLL_METHOD,
            MOUSE_SCROLLING,
        );
        if pointer.domain.get::<String>(MOUSE_SCROLL_METHOD).as_deref() == Some("button") {
            numbered(
                tr!("settings.pointer.scroll-button"),
                MOUSE_SCROLL_BUTTON,
                pointer.count(MOUSE_SCROLL_BUTTON),
                SCROLL_BUTTONS.map(|(button, key)| (button, Label::translated(key))),
            );
            flag(
                pointer,
                tr!("settings.pointer.scroll-button-lock"),
                MOUSE_SCROLL_BUTTON_LOCK,
            );
        }
        degrees(pointer, tr!("settings.pointer.rotation"), MOUSE_ROTATION);
    }
    .gap(Space::SNUG)
}

fn touchpad(pointer: &Pointer) -> impl View {
    column! {
        choice(
            pointer,
            tr!("settings.pointer.touchpad-used"),
            TOUCHPAD_USED,
            TOUCHPAD_USE,
        );
        speed(pointer, tr!("settings.pointer.speed"), TOUCHPAD_SPEED);
        choice(
            pointer,
            tr!("settings.pointer.acceleration"),
            TOUCHPAD_ACCELERATION,
            ACCELERATION,
        );
        flag(pointer, tr!("settings.pointer.natural-scroll"), TOUCHPAD_NATURAL_SCROLL);
        flag(pointer, tr!("settings.pointer.left-handed"), TOUCHPAD_LEFT_HANDED);
        flag(pointer, tr!("settings.pointer.tap"), TOUCHPAD_TAP);
        if pointer.on(TOUCHPAD_TAP) {
            flag(pointer, tr!("settings.pointer.tap-drag"), TOUCHPAD_TAP_DRAG);
            if pointer.on(TOUCHPAD_TAP_DRAG) {
                flag(pointer, tr!("settings.pointer.tap-drag-lock"), TOUCHPAD_TAP_DRAG_LOCK);
            }
            choice(
                pointer,
                tr!("settings.pointer.tap-buttons"),
                TOUCHPAD_TAP_BUTTONS,
                TAP_BUTTONS,
            );
        }
        choice(
            pointer,
            tr!("settings.pointer.click-method"),
            TOUCHPAD_CLICK_METHOD,
            CLICKING,
        );
        choice(
            pointer,
            tr!("settings.pointer.scroll-method"),
            TOUCHPAD_SCROLL_METHOD,
            TOUCHPAD_SCROLLING,
        );
        factor(pointer, tr!("settings.pointer.scroll-factor"), TOUCHPAD_SCROLL_FACTOR);
        flag(
            pointer,
            tr!("settings.pointer.disable-while-typing"),
            TOUCHPAD_DISABLE_WHILE_TYPING,
        );
        flag(
            pointer,
            tr!("settings.pointer.disable-while-pointing"),
            TOUCHPAD_DISABLE_WHILE_POINTING,
        );
        flag(
            pointer,
            tr!("settings.pointer.middle-emulation"),
            TOUCHPAD_MIDDLE_EMULATION,
        );
    }
    .gap(Space::SNUG)
}

fn cursor(pointer: &Pointer) -> impl View {
    let theme = pointer
        .domain
        .non_empty_string(CURSOR_THEME)
        .and_then(|slug| pointer.themes.iter().position(|theme| theme.slug == slug));
    let themes = Arc::clone(&pointer.themes);
    let size = pointer.count(CURSOR_SIZE);

    column! {
        field(
            tr!("settings.pointer.cursor-theme"),
            default_picker(
                tr!("settings.pointer.cursor-inherited"),
                pointer
                    .themes
                    .iter()
                    .map(|theme| Label::from(theme.name.as_str())),
                theme,
                move |pointer: &mut Pointer, index: Option<usize>| {
                    match index.and_then(|index| themes.get(index)) {
                        Some(theme) => pointer.domain.set(CURSOR_THEME, &theme.slug),
                        None => pointer.domain.reset(CURSOR_THEME),
                    }
                },
            ),
        );
        numbered(
            tr!("settings.pointer.cursor-size"),
            CURSOR_SIZE,
            size,
            ladder(&CURSOR_SIZES, size)
                .into_iter()
                .map(|size| (size, Label::from(format!("{size} px")))),
        );
    }
    .gap(Space::SNUG)
}

fn numbered(
    label: Label,
    key: &'static str,
    current: i64,
    options: impl IntoIterator<Item = (i64, Label)>,
) -> impl View {
    let (values, labels): (Vec<i64>, Vec<Label>) = options.into_iter().unzip();
    let selected = values.iter().position(|value| *value == current);
    field(
        label,
        picker(
            selected,
            labels,
            move |pointer: &mut Pointer, index: usize| match values.get(index) {
                Some(value) => pointer.domain.set(key, *value),
                None => pointer.domain.reset(key),
            },
        ),
    )
}

fn speed(pointer: &Pointer, label: Label, key: &'static str) -> impl View {
    sliding(pointer, label, key, -1.0..=1.0, 0.05, percent)
}

fn factor(pointer: &Pointer, label: Label, key: &'static str) -> impl View {
    sliding(pointer, label, key, 0.1..=10.0, 0.1, times)
}

fn degrees(pointer: &Pointer, label: Label, key: &'static str) -> impl View {
    let current = pointer.count(key);
    numbered(
        label,
        key,
        current,
        ladder(&ROTATIONS, current)
            .into_iter()
            .map(|angle| (angle, Label::from(format!("{angle}°")))),
    )
}

fn sliding(
    pointer: &Pointer,
    label: Label,
    key: &'static str,
    range: std::ops::RangeInclusive<f32>,
    step: f32,
    show: fn(f64) -> String,
) -> impl View {
    let value = pointer.domain.get::<f64>(key).unwrap_or_default();
    field(
        label,
        amount(
            value as f32,
            range,
            step,
            show(value),
            move |pointer: &mut Pointer, moved: f32| {
                pointer.domain.set(key, f64::from(moved));
            },
        ),
    )
}

fn ladder(known: &[i64], current: i64) -> Vec<i64> {
    let mut ladder = known.to_vec();
    if !ladder.contains(&current) {
        ladder.push(current);
        ladder.sort_unstable();
    }
    ladder
}

fn percent(speed: f64) -> String {
    format!("{:+.0}%", speed * 100.0)
}

fn times(factor: f64) -> String {
    format!("×{factor:.1}")
}

impl std::fmt::Debug for Pointer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pointer")
            .field("themes", &self.themes.len())
            .finish_non_exhaustive()
    }
}
