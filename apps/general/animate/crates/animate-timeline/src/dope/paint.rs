use std::ops::Range;

use animate_body::Side;
use animate_cast::LiquidCache;
use animate_fluid::Pour;
use animate_hold::{HoldId, HoldLane};
use animate_motion::Tag;
use animate_sheet::{HoldRow, HoldSpan, Lane, Pouring};
use animate_track::Edge;
use animate_transport::Peaks;
use animate_ui::{LEFT_HAND_TINT, PIN_TINT, RIGHT_HAND_TINT};
use rikka_app::prelude::*;
use rikka_model::mmd::FRAMES_PER_SECOND;
use rikka_timeline::{KeyGlyph, KeyShape, Ticks};
use rustc_hash::{FxHashMap, FxHashSet};

use super::hit::row_keys;
use super::{Names, hold, pour};
use crate::layout::{self, Geometry, NAMES_WIDTH, Row};
use crate::state::Board;
use crate::wording;

const TEXT: f32 = 12.5;
const TEXT_LINE: f32 = 17.0;
const LABEL: f32 = 11.0;
const LABEL_LINE: f32 = 14.0;
const GLYPH: f32 = 16.0;
const INSET: f32 = 8.0;
const INDENT: f32 = 30.0;
const KEY: f32 = 10.0;
const GROUP_KEY: f32 = 8.0;
const CROWDED: f32 = 2.0;
const MAJOR_TICK: f32 = 10.0;
const MINOR_TICK: f32 = 5.0;
const WAVE_STEP: f32 = 2.0;
const WAVE_BAR: f32 = 1.5;
const LINE: f32 = 1.0;
const BEYOND_SHADE: f32 = 0.05;
const CACHE_BAND: f32 = 3.0;
const CACHE_TRACK: f32 = 0.18;
const BAR: f32 = 0.3;
const BAR_LIT: f32 = 0.45;
const BAR_RADIUS: f32 = 4.0;
const GRIP: f32 = 2.0;
const DROP: f32 = 10.0;
const CACHED: f32 = 0.75;
const PICKED: Color = Color::rgb(1.0, 0.25, 0.25);
const HOLLOW: f32 = 1.5;
const SHIFTED: f32 = 0.45;
const LABEL_ROOM: f32 = 56.0;
const SMOOTH: Color = Color::rgb(0.3, 0.6, 1.0);
const EASE: Color = Color::rgb(0.35, 0.8, 0.45);
const HIT: Color = Color::rgb(1.0, 0.6, 0.15);
const LINEAR: Color = Color::rgb(0.65, 0.65, 0.7);
const STEP: Color = Color::rgb(0.75, 0.45, 1.0);
const SETTLE: Color = Color::rgb(0.2, 0.8, 0.8);
const LEADING_INSET: f32 = 0.12;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Moving {
    Still {
        over: Option<Edge>,
        hold: Option<(HoldId, Edge)>,
    },
    Shift(i64),
    Pour {
        grip: Edge,
        to: Pour,
    },
    Hold {
        grip: Edge,
        to: HoldSpan,
    },
}

impl Moving {
    fn shift(self) -> i64 {
        match self {
            Self::Shift(by) => by,
            Self::Still { .. } | Self::Pour { .. } | Self::Hold { .. } => 0,
        }
    }

    fn grip(self) -> Option<Edge> {
        match self {
            Self::Still { over, .. } => over,
            Self::Pour { grip, .. } => Some(grip),
            Self::Shift(_) | Self::Hold { .. } => None,
        }
    }

    fn pour(self) -> Option<Pour> {
        match self {
            Self::Pour { to, .. } => Some(to),
            Self::Still { .. } | Self::Shift(_) | Self::Hold { .. } => None,
        }
    }

    fn hold(self, span: HoldSpan) -> (HoldSpan, Option<Edge>) {
        match self {
            Self::Hold { grip, to } if to.id == span.id => (to, Some(grip)),
            Self::Still {
                hold: Some((id, grip)),
                ..
            } if id == span.id => (span, Some(grip)),
            _ => (span, None),
        }
    }
}

pub(crate) fn sheet(
    ctx: &mut PaintCtx<'_>,
    geometry: &Geometry,
    board: &Board,
    names: &Names,
    moving: &Moving,
) {
    let palette = palette();
    let bounds = geometry.bounds;
    let keys = geometry.keys();
    ctx.draw_quad(Quad::new(keys).fill(palette.surface));
    rows(ctx, geometry, board, names, moving);
    ruler(ctx, geometry, board.length);
    cached(ctx, geometry, &board.liquid);
    ctx.draw_quad(
        Quad::new(Rect::new(
            keys.min_x() - LINE,
            bounds.y(),
            LINE,
            bounds.height(),
        ))
        .fill(palette.outline),
    );
    ctx.draw_quad(
        Quad::new(Rect::new(
            bounds.x(),
            keys.min_y() - LINE,
            bounds.width(),
            LINE,
        ))
        .fill(palette.outline),
    );
}

pub(crate) fn selection(ctx: &mut PaintCtx<'_>, geometry: &Geometry, area: Rect) {
    let accent = palette().accent;
    ctx.clipped(geometry.keys(), |ctx| {
        ctx.draw_quad(
            Quad::new(area)
                .fill(accent.scale_alpha(0.12))
                .stroke(LINE, accent),
        );
    });
}

fn ruler(ctx: &mut PaintCtx<'_>, geometry: &Geometry, length: u32) {
    let palette = palette();
    let ruler = geometry.ruler();
    let keys = geometry.keys();
    let end = geometry.x_of(length as f32).max(ruler.min_x());
    if end < ruler.max_x() {
        let beyond = Rect::new(end, ruler.min_y(), ruler.max_x() - end, ruler.height());
        ctx.draw_quad(Quad::new(beyond).fill(palette.overlay.scale_alpha(BEYOND_SHADE)));
    }

    let ticks = Ticks::new(
        geometry.scale.per_frame(),
        FRAMES_PER_SECOND as u32,
        LABEL_ROOM,
    );
    let (from, to) = geometry.lane().frames_shown();
    let bottom = ruler.max_y();
    ctx.clipped(
        Rect::new(
            ruler.x(),
            ruler.y(),
            ruler.width(),
            ruler.height() + keys.height(),
        ),
        |ctx| {
            for tick in ticks.across::<u32>(from, to + 1.0) {
                let x = geometry.x_of(tick.frame as f32);
                let reach = if tick.major { MAJOR_TICK } else { MINOR_TICK };
                ctx.draw_quad(
                    Quad::new(Rect::new(x, bottom - reach, LINE, reach)).fill(palette.text_muted),
                );
                if tick.major {
                    ctx.draw_quad(
                        Quad::new(Rect::new(x, keys.min_y(), LINE, keys.height()))
                            .fill(palette.outline.scale_alpha(0.5)),
                    );
                    ctx.draw_text(
                        Rect::new(x + 3.0, ruler.min_y() + 2.0, LABEL_ROOM, LABEL_LINE),
                        TextRun::new(tick.frame.to_string(), LABEL).no_wrap(),
                        palette.text_muted,
                    );
                }
            }
        },
    );
}

fn cached(ctx: &mut PaintCtx<'_>, geometry: &Geometry, liquid: &LiquidCache) {
    if liquid.ready.is_empty() && liquid.computing.is_none() {
        return;
    }
    let accent = palette().accent;
    let ruler = geometry.ruler();
    let band = |frames: &Range<f32>| {
        let start = geometry.x_of(frames.start);
        let end = geometry.x_of(frames.end);
        Rect::new(start, ruler.max_y() - CACHE_BAND, end - start, CACHE_BAND)
    };
    ctx.clipped(ruler, |ctx| {
        if let Some(computing) = &liquid.computing {
            ctx.draw_quad(Quad::new(band(computing)).fill(accent.scale_alpha(CACHE_TRACK)));
        }
        for frames in &liquid.ready {
            ctx.draw_quad(Quad::new(band(frames)).fill(accent.scale_alpha(CACHED)));
        }
    });
}

fn rows(
    ctx: &mut PaintCtx<'_>,
    geometry: &Geometry,
    board: &Board,
    names: &Names,
    moving: &Moving,
) {
    let palette = palette();
    let keys = geometry.keys();
    let bounds = geometry.bounds;
    let by_lane = picked_by_lane(board);
    let area = Rect::new(
        bounds.x(),
        keys.min_y(),
        keys.max_x() - bounds.x(),
        keys.height(),
    );
    let metrics = TextRun::new("", TEXT).no_wrap();
    let (from, to) = geometry.frames_shown();

    ctx.clipped(area, |ctx| {
        let mut top = geometry.row_top(&board.rows, 0);
        for row in &board.rows {
            let height = row.height();
            let band = Rect::new(bounds.x(), top, bounds.width(), height);
            top += height;
            if band.max_y() < area.min_y() {
                continue;
            }
            if band.min_y() > area.max_y() {
                break;
            }

            if board.chosen == Some(*row) {
                ctx.draw_quad(Quad::new(band).fill(palette.selection.scale_alpha(0.55)));
            }
            ctx.draw_quad(
                Quad::new(Rect::new(band.x(), band.max_y() - LINE, band.width(), LINE))
                    .fill(palette.outline.scale_alpha(0.35)),
            );

            let numbered: Label;
            let titled: std::borrow::Cow<'_, str>;
            let (indent, glyph, name, ink): (f32, Option<MaterialSymbol>, &str, Color) = match *row
            {
                Row::Sound => (
                    INSET,
                    Some(MaterialSymbol::MUSIC_NOTE),
                    names.sound.as_deref().unwrap_or_default(),
                    palette.text,
                ),
                Row::Pour => (
                    INSET,
                    Some(MaterialSymbol::WATER_DROP),
                    names.pour.as_str(),
                    palette.text,
                ),
                Row::Hold(lane) => {
                    let lane = board.sheet.holding()[lane].lane;
                    numbered = names.hold_label(lane);
                    let glyph = match lane {
                        HoldLane::Hand(_) => MaterialSymbol::BACK_HAND,
                        HoldLane::Pin(_) => MaterialSymbol::PUSH_PIN,
                    };
                    (INSET, Some(glyph), numbered.as_str(), palette.text)
                }
                Row::Group(index) => {
                    let group = &board.sheet.groups()[index];
                    let glyph = (group.opens() && !group.kind.stays_open()).then(|| {
                        match board.open.contains(&index) {
                            true => MaterialSymbol::EXPAND_MORE,
                            false => MaterialSymbol::CHEVRON_RIGHT,
                        }
                    });
                    let ink = match layout::is_muted(group) {
                        true => palette.text_muted,
                        false => palette.text,
                    };
                    titled = names.group_name(group);
                    (INSET, glyph, &*titled, ink)
                }
                Row::Track(group, track) => {
                    let owner = &board.sheet.groups()[group];
                    let ink = match layout::is_muted(owner) {
                        true => palette.text_muted,
                        false => palette.text,
                    };
                    (INDENT, None, names.track_name(&owner.tracks[track]), ink)
                }
            };
            if let Some(glyph) = glyph {
                ctx.draw_icon(
                    Rect::new(
                        band.x() + indent,
                        band.y() + (height - GLYPH) / 2.0,
                        GLYPH,
                        GLYPH,
                    ),
                    IconGlyph::from(glyph).spec(GLYPH),
                    palette.text_muted,
                );
            }
            let text_left = band.x() + indent + GLYPH + 4.0;
            let room = bounds.x() + NAMES_WIDTH - text_left - INSET;
            let line = fitted_line(&metrics.clone().content(name), room);
            let mut run = TextRun::new(line.text.as_str(), TEXT).no_wrap();
            if matches!(row, Row::Group(_)) {
                run = run.medium();
            }
            ctx.draw_text(
                Rect::new(
                    text_left,
                    band.y() + (height - TEXT_LINE) / 2.0,
                    line.width + 2.0,
                    TEXT_LINE,
                ),
                run,
                ink,
            );

            ctx.clipped(Rect::new(keys.x(), band.y(), keys.width(), height), |ctx| {
                let (group, size, highlighted) = match *row {
                    Row::Sound => {
                        if let Some(peaks) = &board.peaks {
                            wave(ctx, geometry, peaks, band);
                        }
                        return;
                    }
                    Row::Pour => {
                        if let Some(pouring) = board.sheet.pouring() {
                            pour(ctx, geometry, band, pouring, moving);
                        }
                        return;
                    }
                    Row::Hold(lane) => {
                        let row = &board.sheet.holding()[lane];
                        holds(ctx, geometry, band, row, board.hold, moving);
                        return;
                    }
                    Row::Group(index) => (
                        &board.sheet.groups()[index],
                        GROUP_KEY,
                        palette.text.mix(palette.accent, 0.35),
                    ),
                    Row::Track(group, _) => (&board.sheet.groups()[group], KEY, palette.accent),
                };
                let tint = match layout::is_muted(group) {
                    true => palette.text_muted,
                    false => highlighted,
                };
                let tags = match *row {
                    Row::Track(group, track) => {
                        board.sheet.groups()[group].tracks[track].tags.as_slice()
                    }
                    Row::Group(_) | Row::Sound | Row::Pour | Row::Hold(_) => &[],
                };
                let strip = Strip {
                    keys: row_keys(board, *row),
                    tags,
                    band,
                    size,
                    tint,
                };
                let picked = picked_on(board, &by_lane, *row);
                strip.paint(ctx, geometry, (from, to), moving.shift(), |frame| {
                    picked.contains(&frame)
                });
            });
        }
    });
}

fn holds(
    ctx: &mut PaintCtx<'_>,
    geometry: &Geometry,
    band: Rect,
    row: &HoldRow,
    chosen: Option<HoldId>,
    moving: &Moving,
) {
    let palette = palette();
    let tint = hold_tint(row.lane);
    for span_index in &row.spans {
        let (span, highlighted) = moving.hold(*span_index);
        let bar = hold::bar(geometry, band, span);
        ctx.draw_quad(
            Quad::new(hold::lead(geometry, bar, span.ease))
                .fill(tint.scale_alpha(LEADING_INSET))
                .radius(BAR_RADIUS),
        );
        let edge = match chosen == Some(span.id) {
            true => palette.text,
            false => tint,
        };
        span_body(ctx, bar, tint, edge, highlighted);
        let fading = Rect::new(bar.max_x(), bar.y(), 0.0, bar.height());
        if span.span.end.is_some()
            && let Some(fading) = hold::lead(geometry, fading, span.ease).intersection(&bar)
        {
            ctx.draw_quad(Quad::new(fading).fill(palette.surface.scale_alpha(0.45)));
        }
        span_grips(ctx, bar, tint, highlighted);
    }
}

fn hold_tint(lane: HoldLane) -> Color {
    match lane {
        HoldLane::Hand(Side::Left) => LEFT_HAND_TINT,
        HoldLane::Hand(Side::Right) => RIGHT_HAND_TINT,
        HoldLane::Pin(_) => PIN_TINT,
    }
}

fn span_body(
    ctx: &mut PaintCtx<'_>,
    bar: Rect,
    tint: Color,
    edge: Color,
    highlighted: Option<Edge>,
) {
    let fill = match highlighted {
        Some(_) => BAR_LIT,
        None => BAR,
    };
    ctx.draw_quad(
        Quad::new(bar)
            .fill(tint.scale_alpha(fill))
            .stroke(LINE, edge)
            .radius(BAR_RADIUS),
    );
}

fn span_grips(ctx: &mut PaintCtx<'_>, bar: Rect, tint: Color, highlighted: Option<Edge>) {
    let text = palette().text;
    let roomy = bar.width() >= 4.0 * GRIP;
    for (grip, x) in [
        (Edge::Start, bar.min_x() + GRIP),
        (Edge::End, bar.max_x() - 2.0 * GRIP),
    ] {
        let active_grabs = highlighted == Some(grip);
        if roomy || active_grabs {
            let ink = match active_grabs {
                true => text,
                false => tint,
            };
            ctx.draw_quad(
                Quad::new(Rect::new(
                    x,
                    bar.y() + GRIP,
                    GRIP,
                    bar.height() - 2.0 * GRIP,
                ))
                .fill(ink)
                .radius(GRIP / 2.0),
            );
        }
    }
}

fn pour(
    ctx: &mut PaintCtx<'_>,
    geometry: &Geometry,
    band: Rect,
    pour_row: Pouring,
    moving: &Moving,
) {
    let palette = palette();
    let accent = palette.accent;
    let highlighted = moving.grip();
    let pouring = Pouring {
        pour: moving.pour().unwrap_or(pour_row.pour),
        ..pour_row
    };
    let bar = pour::bar(geometry, band, pouring);
    if !pouring.flows {
        let ring = match highlighted {
            Some(_) => palette.text,
            None => accent,
        };
        ctx.draw_quad(
            Quad::new(Rect::from_center(bar.center(), Size::new(DROP, DROP)))
                .fill(accent)
                .stroke(HOLLOW, ring)
                .radius(DROP / 2.0),
        );
        return;
    }
    span_body(ctx, bar, accent, accent, highlighted);
    span_grips(ctx, bar, accent, highlighted);
    let run = TextRun::new(wording::seconds(pouring.pour.seconds()), LABEL)
        .medium()
        .no_wrap();
    let width = measure_text(&run, None).width;
    let (x, ink) = match bar.width() >= width + 4.0 * GRIP + 2.0 * INSET {
        true => (bar.min_x() + 3.0 * GRIP, palette.text),
        false => (bar.max_x() + INSET, palette.text_muted),
    };
    ctx.draw_text(
        Rect::new(
            x,
            band.center().y - LABEL_LINE / 2.0,
            width + 2.0,
            LABEL_LINE,
        ),
        run,
        ink,
    );
}

struct Strip<'a> {
    keys: &'a [u32],
    tags: &'a [Tag],
    band: Rect,
    size: f32,
    tint: Color,
}

impl Strip<'_> {
    fn paint(
        &self,
        ctx: &mut PaintCtx<'_>,
        geometry: &Geometry,
        (from, to): (f32, f32),
        shift: i64,
        picked: impl Fn(u32) -> bool,
    ) {
        let keys = self.keys;
        let start = keys.partition_point(|key| (*key as f32) < from - 1.0);
        let middle = self.band.center().y;
        let mut last = f32::NEG_INFINITY;
        for (at, key) in keys.iter().enumerate().skip(start) {
            let glyph = KeyGlyph::whole(shape(self.tags.get(at).copied()));
            if *key as f32 > to + 1.0 {
                break;
            }
            let x = geometry.x_of(*key as f32);
            let chosen = picked(*key);
            if x - last < CROWDED && !chosen {
                continue;
            }
            last = x;
            let tint = match chosen {
                true => PICKED,
                false => self.tags.get(at).copied().map_or(self.tint, tinted),
            };
            glyph.paint(ctx, Point::new(x, middle), self.size, tint);
            if chosen && shift != 0 {
                let shifted_x = geometry.x_of(*key as f32 + shift as f32);
                glyph.paint(
                    ctx,
                    Point::new(shifted_x, middle),
                    self.size,
                    PICKED.scale_alpha(SHIFTED),
                );
            }
        }
    }
}

fn shape(tag: Option<Tag>) -> KeyShape {
    match tag {
        None => KeyShape::Square,
        Some(Tag::Ease) => KeyShape::Hourglass,
        Some(Tag::Step) => KeyShape::Outline,
        Some(Tag::Smooth) => KeyShape::Circle,
        Some(Tag::Settle) => KeyShape::Ring,
        Some(Tag::Hit) => KeyShape::Bar,
        Some(Tag::Linear) => KeyShape::Diamond,
    }
}

fn tinted(tag: Tag) -> Color {
    match tag {
        Tag::Smooth => SMOOTH,
        Tag::Ease => EASE,
        Tag::Hit => HIT,
        Tag::Linear => LINEAR,
        Tag::Step => STEP,
        Tag::Settle => SETTLE,
    }
}

fn wave(ctx: &mut PaintCtx<'_>, geometry: &Geometry, peaks: &Peaks, band: Rect) {
    let tint = palette().accent.scale_alpha(0.75);
    let keys = geometry.keys();
    let middle = band.center().y;
    let half = band.height() / 2.0 - 3.0;
    let seconds = |x: f32| geometry.frame_at(x) / FRAMES_PER_SECOND;
    let mut x = keys.min_x().max(geometry.x_of(0.0));
    while x < keys.max_x() {
        if let Some((low, high)) = peaks.within(seconds(x), seconds(x + WAVE_STEP)) {
            let top = middle - high * half;
            let bottom = middle - low * half;
            ctx.draw_quad(
                Quad::new(Rect::new(x, top, WAVE_BAR, (bottom - top).max(LINE))).fill(tint),
            );
        }
        x += WAVE_STEP;
    }
}

fn picked_by_lane(board: &Board) -> FxHashMap<&Lane, Vec<u32>> {
    let mut by_lane: FxHashMap<&Lane, Vec<u32>> = FxHashMap::default();
    for id in &board.picked {
        by_lane.entry(&id.lane).or_default().push(id.frame);
    }
    by_lane
}

fn picked_on(board: &Board, by_lane: &FxHashMap<&Lane, Vec<u32>>, row: Row) -> FxHashSet<u32> {
    let frames = |lane: &Lane| by_lane.get(lane).into_iter().flatten().copied();
    match row {
        Row::Sound | Row::Pour | Row::Hold(_) => FxHashSet::default(),
        Row::Group(group) => board.sheet.groups()[group]
            .lanes()
            .flat_map(frames)
            .collect(),
        Row::Track(group, track) => {
            frames(&board.sheet.groups()[group].tracks[track].lane).collect()
        }
    }
}
