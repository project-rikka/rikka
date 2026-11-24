use std::mem::discriminant;

use rikka_sound::Spectrum;
use rikka_theme::{Corner, Palette};
use rikka_types::Color;
use rikka_ui::{
    Align, Axis, Element, Length, MaterialSymbol, Space, Styled, TextStyled, View, palette, text,
};
use rikka_widgets::{column, container, icon, row, spacer};
use shell_feeds::{art, cover, percent, waveform};
use shell_kit::taskbar::DEPTH;

use crate::axis;
use crate::glide::{glide, grows, settles};
use crate::island::clock::span;
use crate::island::cover_transition::{CoverFrame, CoverTarget};
use crate::island::face::{self, Face, face};
use crate::island::live::{self, Live};
use crate::island::notice::{Tone, sensor_symbol, sensor_tone};
use crate::island::pill::{self, bubble, pill};
use crate::island::state::Island;
use crate::tasks::{TaskList, task_list};

const BUBBLES: usize = 2;
const MARK: f32 = 16.0;
const ART: f32 = 20.0;
const WAVE_BARS: usize = 5;
const WAVE_HEIGHT: f32 = 14.0;
const TITLE: f32 = 220.0;
const TEXT_INSET: f32 = 12.0;
const FLANK: f32 = 360.0;
const SIDE_FLANK: f32 = 260.0;
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

fn seat(live: &Live, axis: Axis) -> f32 {
    match (live, axis) {
        (Live::Media(_), _) => 26.0,
        (Live::Sensor { .. }, _) | (_, Axis::Vertical) => MARK,
        (Live::Recording(_), Axis::Horizontal) => 46.0,
        (Live::LowBattery { .. } | Live::Charging { .. }, Axis::Horizontal) => 36.0,
    }
}

fn flank(axis: Axis) -> f32 {
    match axis {
        Axis::Horizontal => FLANK,
        Axis::Vertical => SIDE_FLANK,
    }
}

pub(crate) fn compact(island: &Island, live_activities: &[Live], seated: bool) -> Face {
    let axis = island.dock().axis();
    let lead = live_activities.first();
    let mode = Mode::of(island);
    let main = pill(
        content(island, lead, mode, axis),
        axis::inset(axis, mode.lead_inset(lead), mode.trail_inset(lead)),
        axis,
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
                axis,
                bubble(live_bubble(live), axis),
                grows(),
                (BUBBLE, discriminant(live)),
            )
        })
        .collect();
    let children = mirror(bubbles.len())
        .map(|length| {
            let size = axis::size(axis, length, 0.0);
            spacer()
                .width(size.width)
                .height(size.height)
                .grow(0.0)
                .build()
                .keyed(MIRROR)
        })
        .into_iter()
        .chain(std::iter::once(main))
        .chain(bubbles)
        .collect();
    face(children, seated, axis)
}

fn content(island: &Island, lead: Option<&Live>, mode: Mode, axis: Axis) -> Element {
    let tasks = island.tasks();
    let shown = match mode {
        Mode::Idle => &[][..],
        Mode::Tasks => tasks.shown(),
    };
    let list = task_list(
        TaskList {
            groups: shown,
            space: tasks.board().showing,
            tiles: tasks.tiles(),
            longest: (island.dock().length() - flank(axis) * 2.0).max(0.0),
            axis,
        },
        Island::touched,
    )
    .keyed(LIST);
    let leading =
        lead.map(|live| {
            glide(
                axis,
                container(beside(leading(live), live, Side::Lead, mode, axis))
                    .padding(axis::inset(axis, 0.0, SPACING.px())),
                grows(),
                (Side::Lead, mode, discriminant(live)),
            )
        });
    let middle = (mode == Mode::Idle)
        .then(|| middle(island, lead, axis))
        .flatten()
        .map(|middle| glide(axis, middle, settles(), Middle::of(lead)));
    let trailing = lead.and_then(|live| {
        let trail = trailing(live, island, island.spectrum(), axis)?;
        Some(glide(
            axis,
            container(beside(trail, live, Side::Trail, mode, axis)).padding(axis::inset(
                axis,
                SPACING.px(),
                0.0,
            )),
            grows(),
            (Side::Trail, mode, discriminant(live)),
        ))
    });
    axis::line(axis, leading)
        .extend(middle)
        .push(list)
        .extend(trailing)
        .align_items(Align::Center)
        .build()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Side {
    Lead,
    Trail,
}

fn beside(child: Element, live: &Live, side: Side, mode: Mode, axis: Axis) -> Element {
    let align = match side {
        Side::Lead => Align::Start,
        Side::Trail => Align::End,
    };
    match (mode, axis) {
        (Mode::Idle, _) => child,
        (Mode::Tasks, Axis::Horizontal) => container(child)
            .align_x(align)
            .align_y(Align::Center)
            .on_hover(Island::aim_live)
            .width(Length::px(seat(live, axis)))
            .height(Length::FILL)
            .shrink(0.0)
            .build(),
        (Mode::Tasks, Axis::Vertical) => container(child)
            .align_x(Align::Center)
            .align_y(align)
            .on_hover(Island::aim_live)
            .width(Length::FILL)
            .height(Length::px(seat(live, axis)))
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
    let before = mirror(bubbles).map_or(0.0, |length| length + face::GAP.px());
    let inset = Mode::of(island).lead_inset(live_activities.first());
    let axis = island.dock().axis();
    let frame = COVER.inset(axis.point(before + inset, (DEPTH - ART) / 2.0));
    Some(CoverTarget::new(frame, cover(player.track.as_ref())))
}

fn middle(island: &Island, lead: Option<&Live>, axis: Axis) -> Option<Element> {
    let clock = island.clock();
    match (lead, axis) {
        (Some(Live::Media(_)), Axis::Vertical) => None,
        (_, Axis::Vertical) => Some(
            column! {
                text(clock.hour()).label().no_wrap();
                text(clock.minute()).label().no_wrap();
            }
            .align_items(Align::Center)
            .build(),
        ),
        (Some(Live::Media(player)), Axis::Horizontal) => Some(
            text(live::title(player))
                .label()
                .no_wrap()
                .max_width(Length::px(TITLE))
                .build(),
        ),
        (Some(_), Axis::Horizontal) => Some(text(clock.time()).label().no_wrap().build()),
        (None, Axis::Horizontal) => Some(
            row! {
                text(clock.date()).label().no_wrap();
                text(clock.time()).label().no_wrap();
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .build(),
        ),
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

fn trailing(
    live: &Live,
    island: &Island,
    spectrum: Option<Spectrum>,
    axis: Axis,
) -> Option<Element> {
    let palette = palette();
    match (live, axis) {
        (Live::Media(_), _) => Some(
            waveform(spectrum, WAVE_BARS, WAVE_HEIGHT)
                .color(palette.accent)
                .build(),
        ),
        (Live::Sensor { sensor, .. }, _) => Some(mark(sensor_symbol(*sensor), palette.text_muted)),
        (Live::Recording(_) | Live::LowBattery { .. } | Live::Charging { .. }, Axis::Vertical) => {
            None
        }
        (Live::Recording(_), Axis::Horizontal) => Some(
            text(span(island.elapsed()))
                .label()
                .color(palette.danger)
                .no_wrap()
                .build(),
        ),
        (Live::LowBattery { charge }, Axis::Horizontal) => Some(
            text(percent(*charge))
                .label()
                .color(palette.danger)
                .no_wrap()
                .build(),
        ),
        (Live::Charging { charge, .. }, Axis::Horizontal) => {
            Some(text(percent(*charge)).label().no_wrap().build())
        }
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
