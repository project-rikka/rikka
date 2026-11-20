use rikka_seam::{Payload, Request, SeamClient, SeamError};
use rikka_session::following;
use rikka_session::toplevels::{self, ToplevelsClient};
use rikka_theme::Corner;
use rikka_ui::{
    Align, Edge, Element, Inset, Label, Length, MaterialSymbol, Motion, Property, Space, State,
    Styled, TextStyled, Transition, View, is_exiting, offset, palette, slide, spawn, stream, text,
    tr,
};
use rikka_widgets::{
    ButtonVariant, ControlSize, button, column, container, dropdown, icon, menu, menu_item, row_of,
    rule_upright, spinner,
};
use shell_kit::EXIT_DURATION;

use crate::aim::{Aim, Aimed, Mode, Target};
use crate::scan::Codes;
use crate::service::{AIM, CANCEL, CAPABILITY, CHANGED};
use crate::windows::Windows;

pub(crate) const WIDTH: f32 = CONTENT;
pub(crate) const HEIGHT: f32 = 56.0;

pub(crate) const MARGIN: f32 = 12.0;

const INSET: f32 = 8.0;
pub(crate) const GAP: f32 = 4.0;
const HAIR: f32 = 1.0;
const DIVIDER_HEIGHT: f32 = 22.0;
const ICON: f32 = 20.0;
const TARGETS: f32 = 140.0;

const SIZE: ControlSize = ControlSize::Small;

const BUTTON: f32 = SIZE.height();

const MODES: f32 = Mode::ALL.len() as f32;

const UNDER: f32 = INSET + BUTTON * MODES + HAIR + GAP * (MODES + 1.0);

const CONTENT: f32 =
    INSET * 2.0 + BUTTON * (MODES + 1.0) + HAIR * 2.0 + TARGETS + GAP * (MODES + 3.0);

#[derive(State)]
pub(super) struct Toolbar {
    seam: SeamClient,
    aimed: Aimed,
    windows: Windows,
    codes: Codes,
    open: bool,
}

impl Toolbar {
    #[must_use]
    pub(crate) fn new(seam: SeamClient, aimed: Aimed, windows: Windows, codes: Codes) -> Toolbar {
        Toolbar {
            seam,
            aimed,
            windows,
            codes,
            open: false,
        }
    }

    pub(crate) fn start(&mut self) {
        spawn(
            ToplevelsClient::new(self.seam.clone()).state(),
            |toolbar: &mut Toolbar, answered: Result<Payload, SeamError>| match answered
                .and_then(|payload| payload.decode::<toplevels::State>())
            {
                Ok(state) => toolbar.windows.follow(&state),
                Err(error) => {
                    tracing::warn!("failed to read window list for picker: {error}");
                }
            },
        );
        let announced = following::changes(&self.seam, CAPABILITY, CHANGED);
        stream(announced, |_: &mut Toolbar, ()| {});
    }

    pub(crate) fn leave(&mut self) {
        self.open = false;
    }

    fn set_open(&mut self, open: bool) {
        self.open = open;
    }

    fn aim(&mut self, aim: Aim) {
        self.open = false;
        self.aimed.set(aim);
        self.seam.notify(Request::new(CAPABILITY, AIM).encode(&aim));
    }

    fn close(&mut self) {
        self.seam.notify(Request::new(CAPABILITY, CANCEL));
    }
}

pub(crate) fn view(toolbar: &Toolbar) -> impl View {
    column! {
        if !is_exiting() {
            container(stack(toolbar)).transition(past_edge());
        }
    }
    .padding(Inset::edges(MARGIN, 0.0, 0.0, 0.0))
    .min_width(WIDTH)
    .min_height(MARGIN + HEIGHT)
}

fn past_edge() -> Transition {
    (slide(Edge::Top) + offset(0.0, -MARGIN))
        .exit(Motion::exit(EXIT_DURATION).curve(Property::Spatial))
}

fn stack(toolbar: &Toolbar) -> impl View {
    let aim = toolbar.aimed.get();
    column! {
        bar(toolbar, aim);
        if toolbar.open && aim.mode != Mode::Scan {
            container(list(aim)).padding(Inset::edges(0.0, 0.0, 0.0, UNDER));
        }
    }
    .gap(Space::TIGHT)
}

fn mode_button(mode: Mode, aim: Aim) -> impl View {
    let chosen = aim.mode == mode;
    let symbol = match mode {
        Mode::Picture => MaterialSymbol::PHOTO_CAMERA,
        Mode::Video => MaterialSymbol::VIDEOCAM,
        Mode::Translate => MaterialSymbol::TRANSLATE,
        Mode::Scan => MaterialSymbol::QR_CODE_SCANNER,
    };
    let ink = match chosen {
        true => palette().accent,
        false => palette().text,
    };
    button(icon(symbol).extent(Length::px(ICON)).color(ink))
        .variant(match chosen {
            true => ButtonVariant::Secondary,
            false => ButtonVariant::Ghost,
        })
        .control_size(SIZE)
        .icon_only()
        .on_press(move |toolbar: &mut Toolbar| toolbar.aim(Aim { mode, ..aim }))
}

fn bar(toolbar: &Toolbar, aim: Aim) -> impl View {
    let modes = Mode::ALL.map(|mode| mode_button(mode, aim).build());
    let aiming = match aim.mode {
        Mode::Scan => status(toolbar),
        _ => targets(toolbar.open, aim),
    };
    let row = row_of(modes)
        .push(rule_upright().height(DIVIDER_HEIGHT))
        .push(aiming)
        .push(rule_upright().height(DIVIDER_HEIGHT))
        .push(
            button(icon(MaterialSymbol::CLOSE).extent(Length::px(ICON)))
                .ghost()
                .control_size(SIZE)
                .icon_only()
                .on_press(Toolbar::close),
        );

    container(row.gap(Space::TIGHT).align_items(Align::Center))
        .surface()
        .radius(Corner::Panel)
        .border(HAIR, palette().outline)
        .pane()
        .padding(INSET)
}

fn targets(open: bool, aim: Aim) -> Element {
    let at = Target::ALL
        .iter()
        .position(|target| *target == aim.target)
        .unwrap_or_default();
    dropdown(at)
        .options(Target::ALL.map(label))
        .control_size(SIZE)
        .open(open)
        .on_toggle(Toolbar::set_open)
        .width(TARGETS)
        .build()
}

fn status(toolbar: &Toolbar) -> Element {
    let decoded = toolbar.codes.read();
    let looking = decoded.looking();
    let shown_text = match looking {
        true => tr!("shell.capture.scan.looking"),
        false if decoded.codes.is_empty() && decoded.screened => tr!("shell.capture.scan.none"),
        false => tr!("shell.capture.scan.found", count = decoded.codes.len()),
    };
    let spinning = looking.then(|| spinner().extent(Length::px(ICON)).build());
    row_of(
        spinning
            .into_iter()
            .chain([text(shown_text).label().no_wrap().build()]),
    )
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .padding(Inset::xy(GAP, 0.0))
    .min_width(TARGETS)
    .build()
}

fn list(aim: Aim) -> impl View {
    menu()
        .items(Target::ALL.map(|target| {
            menu_item(label(target))
                .icon(symbol(target))
                .selected(target == aim.target)
        }))
        .control_size(SIZE)
        .on_dismiss(|toolbar: &mut Toolbar| toolbar.set_open(false))
        .on_activate(
            move |toolbar: &mut Toolbar, index: usize| match Target::ALL.get(index) {
                Some(target) => toolbar.aim(Aim {
                    target: *target,
                    ..aim
                }),
                None => toolbar.set_open(false),
            },
        )
        .width(TARGETS)
}

fn label(target: Target) -> Label {
    match target {
        Target::Region => tr!("shell.capture.target.region"),
        Target::Window => tr!("shell.capture.target.window"),
        Target::Screen => tr!("shell.capture.target.screen"),
    }
}

fn symbol(target: Target) -> MaterialSymbol {
    match target {
        Target::Region => MaterialSymbol::CROP_FREE,
        Target::Window => MaterialSymbol::SELECT_WINDOW,
        Target::Screen => MaterialSymbol::FULLSCREEN,
    }
}

impl std::fmt::Debug for Toolbar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Toolbar")
            .field("aim", &self.aimed.get())
            .field("open", &self.open)
            .finish_non_exhaustive()
    }
}

const _: () = {
    assert!(
        UNDER + TARGETS <= CONTENT,
        "the list runs off the end of the bar"
    );
};
