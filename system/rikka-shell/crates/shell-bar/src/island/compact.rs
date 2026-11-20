use std::mem::discriminant;

use rikka_sound::Spectrum;
use rikka_theme::{Corner, Palette};
use rikka_types::{Color, Point};
use rikka_ui::{
    Align, Element, Inset, Length, MaterialSymbol, Space, Styled, TextStyled, View, palette, text,
};
use rikka_widgets::{container, icon, row, spacer};
use shell_feeds::{art, cover, percent, waveform};
use shell_kit::taskbar::Shape;

use crate::glide::{glide, grows, settles};
use crate::island::clock::span;
use crate::island::cover_transition::{CoverFrame, CoverTarget};
use crate::island::face::{self, Face, face};
use crate::island::live::{self, Live};
use crate::island::notice::{Tone, sensor_symbol, sensor_tone};
use crate::island::pill::{self, bubble, pill};
use crate::island::state::Island;
use crate::tasks::task_list;

const BUBBLES: usize = 2;
const MARK: f32 = 16.0;
const ART: f32 = 20.0;
const WAVE_BARS: usize = 5;
const WAVE_HEIGHT: f32 = 14.0;
const TITLE: f32 = 220.0;
const TEXT_INSET: f32 = 12.0;
const FLANK: f32 = 360.0;
const BARE: f32 = 2.0;
const SPACING: Space = Space::SNUG;

const PILL: &str = "island.pill";
const MIRROR: &str = "island.mirror";
const LIST: &str = "island.tasks";
const BUBBLE: &str = "island.bubble";

const COVER: CoverFrame = CoverFrame::new(ART, 6.0);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Mode {
    Idle,
    Tasks,
}

impl Mode {
    #[must_use]
    pub(crate) fn of(island: &Island) -> Mode {
        match island.tasks().shown().is_empty() {
            true => Mode::Idle,
            false => Mode::Tasks,
        }
    }

    fn lead_inset(self, lead: Option<&Live>) -> f32 {
        match (self, lead) {
            (_, Some(Live::Media(_))) => (pill::HEIGHT - ART) / 2.0,
            (_, Some(_)) => (pill::HEIGHT - MARK) / 2.0,
            (Mode::Idle, None) => TEXT_INSET,
            (Mode::Tasks, None) => BARE,
        }
    }

    fn trail_inset(self, lead: Option<&Live>) -> f32 {
        match (self, lead) {
            (Mode::Idle, Some(Live::Media(_))) => (pill::HEIGHT - WAVE_HEIGHT) / 2.0,
            (Mode::Idle, Some(Live::Sensor { .. })) => (pill::HEIGHT - MARK) / 2.0,
            (Mode::Idle, Some(_) | None) => TEXT_INSET,
            (Mode::Tasks, _) => self.lead_inset(lead),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Middle {
    Title,
    Time,
    Date,
}

impl Middle {
    fn of(lead: Option<&Live>) -> Middle {
        match lead {
            Some(Live::Media(_)) => Middle::Title,
            Some(_) => Middle::Time,
            None => Middle::Date,
        }
    }
}

fn seat_width(live: &Live) -> f32 {
    match live {
        Live::Media(_) => 26.0,
        Live::Recording(_) => 46.0,
        Live::Sensor { .. } => MARK,
        Live::LowBattery { .. } | Live::Charging { .. } => 36.0,
    }
}

pub(crate) fn compact(island: &Island, live_activities: &[Live], seated: bool) -> Face {
    let lead = live_activities.first();
    let mode = Mode::of(island);
    let main = pill(
        content(island, lead, mode),
        Inset::exact(0.0, mode.trail_inset(lead), 0.0, mode.lead_inset(lead)),
    );
    let main = container(main)
        .on_hover(Island::hover_face)
        .build()
        .keyed(PILL);
    let bubbles: Vec<Element> = live_activities
        .iter()
        .skip(1)
        .take(BUBBLES)
        .map(|live| {
            glide(
                bubble(live_bubble(live)),
                grows(),
                (BUBBLE, discriminant(live)),
            )
        })
        .collect();
    let children = mirror(bubbles.len())
        .map(|width| spacer().width(width).grow(0.0).build().keyed(MIRROR))
        .into_iter()
        .chain(std::iter::once(main))
        .chain(bubbles)
        .collect();
    face(children, seated)
}

fn content(island: &Island, lead: Option<&Live>, mode: Mode) -> Element {
    let tasks = island.tasks();
    let shown = match mode {
        Mode::Idle => &[][..],
        Mode::Tasks => tasks.shown(),
    };
    let list = task_list(
        shown,
        tasks.board().showing,
        tasks.tiles(),
        (island.width() - FLANK * 2.0).max(0.0),
        Island::touched,
    )
    .keyed(LIST);
    row! {
        if let Some(live) = lead {
            glide(
                container(beside(leading(live), live, Side::Lead, mode))
                    .padding(Inset::exact(0.0, SPACING.px(), 0.0, 0.0)),
                grows(),
                (Side::Lead, mode, discriminant(live)),
            );
        }
        if mode == Mode::Idle {
            glide(middle(island, lead), settles(), Middle::of(lead));
        }
        list;
        if let Some(live) = lead {
            glide(
                container(beside(trailing(live, island, island.spectrum()), live, Side::Trail, mode))
                    .padding(Inset::exact(0.0, 0.0, 0.0, SPACING.px())),
                grows(),
                (Side::Trail, mode, discriminant(live)),
            );
        }
    }
    .align_items(Align::Center)
    .build()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Side {
    Lead,
    Trail,
}

fn beside(child: Element, live: &Live, side: Side, mode: Mode) -> Element {
    let align = match side {
        Side::Lead => Align::Start,
        Side::Trail => Align::End,
    };
    match mode {
        Mode::Idle => child,
        Mode::Tasks => container(child)
            .align_x(align)
            .align_y(Align::Center)
            .on_hover(Island::aim_live)
            .width(Length::px(seat_width(live)))
            .height(Length::FILL)
            .shrink(0.0)
            .build(),
    }
}

fn mirror(bubbles: usize) -> Option<f32> {
    (bubbles > 0).then(|| bubbles as f32 * pill::HEIGHT + (bubbles - 1) as f32 * face::GAP.px())
}

#[must_use]
pub(crate) fn cover_target(island: &Island, live_activities: &[Live]) -> Option<CoverTarget> {
    let Some(Live::Media(player)) = live_activities.first() else {
        return None;
    };
    let bubbles = live_activities.len().saturating_sub(1).min(BUBBLES);
    let before = mirror(bubbles).map_or(0.0, |width| width + face::GAP.px());
    let inset = Mode::of(island).lead_inset(live_activities.first());
    let frame = COVER.inset(Point::new(before + inset, (Shape.reserved() - ART) / 2.0));
    Some(CoverTarget::new(frame, cover(player.track.as_ref())))
}

fn middle(island: &Island, lead: Option<&Live>) -> Element {
    match lead {
        Some(Live::Media(player)) => text(live::title(player))
            .label()
            .no_wrap()
            .max_width(Length::px(TITLE))
            .build(),
        Some(_) => text(island.clock().time()).label().no_wrap().build(),
        None => {
            let clock = island.clock();
            row! {
                text(clock.date()).label().no_wrap();
                text(clock.time()).label().no_wrap();
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .build()
        }
    }
}

fn leading(live: &Live) -> Element {
    let palette = palette();
    match live {
        Live::Media(_) => COVER.seat().build(),
        Live::Recording(_) => mark(MaterialSymbol::FIBER_MANUAL_RECORD, palette.danger),
        Live::Sensor { sensor, .. } => mark(
            MaterialSymbol::FIBER_MANUAL_RECORD,
            tinted(sensor_tone(*sensor), palette),
        ),
        Live::LowBattery { .. } => mark(MaterialSymbol::BATTERY_ALERT, palette.danger),
        Live::Charging { .. } => mark(MaterialSymbol::BOLT, palette.success),
    }
}

fn trailing(live: &Live, island: &Island, spectrum: Option<Spectrum>) -> Element {
    let palette = palette();
    match live {
        Live::Media(_) => waveform(spectrum, WAVE_BARS, WAVE_HEIGHT)
            .color(palette.accent)
            .build(),
        Live::Recording(_) => text(span(island.elapsed()))
            .label()
            .color(palette.danger)
            .no_wrap()
            .build(),
        Live::Sensor { sensor, .. } => mark(sensor_symbol(*sensor), palette.text_muted),
        Live::LowBattery { charge } => text(percent(*charge))
            .label()
            .color(palette.danger)
            .no_wrap()
            .build(),
        Live::Charging { charge, .. } => text(percent(*charge)).label().no_wrap().build(),
    }
}

fn live_bubble(live: &Live) -> Element {
    let palette = palette();
    match live {
        Live::Media(player) => art(player.track.as_ref(), ART, Corner::Item),
        Live::Recording(_) => mark(MaterialSymbol::FIBER_MANUAL_RECORD, palette.danger),
        Live::Sensor { sensor, .. } => mark(
            sensor_symbol(*sensor),
            tinted(sensor_tone(*sensor), palette),
        ),
        Live::LowBattery { .. } => mark(MaterialSymbol::BATTERY_ALERT, palette.danger),
        Live::Charging { .. } => mark(MaterialSymbol::BOLT, palette.success),
    }
}

fn mark(symbol: MaterialSymbol, color: Color) -> Element {
    icon(symbol)
        .extent(Length::px(MARK))
        .filled()
        .color(color)
        .build()
}

pub(crate) fn tinted(tone: Tone, palette: Palette) -> Color {
    match tone {
        Tone::Plain => palette.text,
        Tone::Good => palette.success,
        Tone::Warning => palette.warning,
        Tone::Danger => palette.danger,
    }
}
