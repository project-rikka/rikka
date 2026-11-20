use std::path::Path;
use std::sync::Arc;

use rikka_theme::Corner;
use rikka_types::{CornerRadii, Point, PointerButton, Rect, RoundedRect, ScrollDelta, Size};
use rikka_ui::{
    Curve, Easing, Element, Event, EventCtx, Handler, IconGlyph, ImageSource, IntoHandler, Label,
    Layout, Length, MaterialSymbol, PaintCtx, Quad, Spring, TextAlign, TextRun, VideoController,
    View, Widget, palette,
};
use rikka_widgets::ImageFit;

use crate::provider::Preview;

const TILE: Size = Size::new(196.0, 112.0);

const STRIDE: f32 = 172.0;

const SMALLEST: f32 = 0.58;
pub(crate) const REACH: f32 = 2.8;

const FADE: f32 = 0.9;

const RADIUS: f32 = Corner::Item.radius();
const RING: f32 = 2.0;

const LABEL_SIZE: f32 = 13.0;
const LABEL_GAP: f32 = 10.0;
const LABEL_HEIGHT: f32 = 18.0;

const BADGE: f32 = 16.0;
const BADGE_INSET: f32 = 8.0;

const PADDING: f32 = 22.0;

const HEIGHT: f32 = PADDING * 2.0 + TILE.height + LABEL_GAP + LABEL_HEIGHT;

const SETTLE: Curve = Curve::Spring(Spring::DEFAULT_SPATIAL);

pub(super) struct Gallery {
    tiles: Vec<Tile>,
    selected: usize,
    turn: Turn,
    on_select: Handler<(usize,)>,
    on_activate: Handler<(usize,)>,
}

pub(super) struct Tile {
    pub(crate) label: Label,
    pub(crate) preview: Preview,
    pub(crate) in_use: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Turn {
    serial: u64,
    by: i64,
}

impl Turn {
    #[must_use]
    pub(crate) fn then(self, by: i64) -> Turn {
        Turn {
            serial: self.serial.wrapping_add(1),
            by,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Spot {
    place: i64,
    index: usize,
}

#[derive(Debug, Default)]
pub(crate) struct GalleryState {
    aim: Option<i64>,
    incoming: u64,
    playing: Option<(Arc<Path>, VideoController)>,
    showing: Vec<Spot>,
    placed: Vec<(Spot, Rect)>,
}

pub(crate) fn gallery<Selected, Activated>(
    tiles: Vec<Tile>,
    selected: usize,
    on_select: impl IntoHandler<(usize,), (), Selected>,
    on_activate: impl IntoHandler<(usize,), (), Activated>,
) -> Gallery {
    Gallery {
        tiles,
        selected,
        turn: Turn::default(),
        on_select: on_select.into_handler(),
        on_activate: on_activate.into_handler(),
    }
}

impl Widget for Gallery {
    type State = GalleryState;

    fn layout(&self) -> Layout {
        Layout::row().size(Length::FILL, Length::px(HEIGHT))
    }

    fn paint(&self, state: &mut GalleryState, ctx: &mut PaintCtx<'_>) {
        let area = ctx.bounds();
        if area.is_empty() || self.tiles.is_empty() {
            return;
        }
        let aim = self.aimed(state);
        let at = ctx.animate_curve("at", aim as f32, SETTLE);
        let axis = Point::new(area.center().x, area.min_y() + PADDING + TILE.height / 2.0);

        state.placed.clear();
        state.showing.clear();
        state.showing.extend(self.within(at));
        state
            .showing
            .sort_by(|a, b| offset(*b, at).abs().total_cmp(&offset(*a, at).abs()));

        let playing = self.playing(state);
        let GalleryState {
            showing, placed, ..
        } = state;
        ctx.clipped(RoundedRect::rect(area), |ctx| {
            for spot in showing.iter().copied() {
                let offset = offset(spot, at);
                let rect = place(axis, offset);
                placed.push((spot, rect));
                let centre = spot.place == aim;
                ctx.with_opacity(fade(offset), |ctx| {
                    self.draw(spot.index, rect, centre, playing.as_ref(), ctx);
                });
            }
        });
    }

    fn relocated(&self, state: &mut GalleryState, by: Point) {
        for (_, rect) in &mut state.placed {
            *rect = rect.translate(by);
        }
    }

    fn event(&self, state: &mut GalleryState, event: &Event, ctx: &mut EventCtx<'_>) {
        match event {
            Event::PointerPressed {
                button: PointerButton::Primary,
                position,
                ..
            } => {
                let Some(spot) = at_point(&state.placed, *position) else {
                    return;
                };
                ctx.set_handled();
                if state.aim == Some(spot.place) {
                    ctx.call(&self.on_activate, (spot.index,));
                } else {
                    self.turn_to(spot.place, state, ctx);
                }
            }
            Event::Scroll { delta, .. } => {
                let Some(step) = step(*delta) else {
                    return;
                };
                if self.tiles.is_empty() {
                    return;
                }
                ctx.set_handled();
                let aim = self.aimed(state) + step;
                self.turn_to(aim, state, ctx);
            }
            _ => {}
        }
    }

    fn debug_name(&self) -> &'static str {
        "Gallery"
    }
}

impl Gallery {
    #[must_use]
    pub(crate) fn turned(mut self, turn: Turn) -> Gallery {
        self.turn = turn;
        self
    }

    fn playing(&self, state: &mut GalleryState) -> Option<VideoController> {
        let path = match self.tiles.get(self.selected).map(|tile| &tile.preview) {
            Some(Preview::Motion(path)) => path,
            _ => {
                state.playing = None;
                return None;
            }
        };
        if state
            .playing
            .as_ref()
            .is_none_or(|(playing, _)| playing != path)
        {
            let controller = VideoController::open(path.to_path_buf());
            controller.set_looping(true);
            controller.play();
            state.playing = Some((Arc::clone(path), controller));
        }
        state
            .playing
            .as_ref()
            .map(|(_, controller)| controller.clone())
    }

    fn aimed(&self, state: &mut GalleryState) -> i64 {
        let count = self.tiles.len() as i64;
        let selected = self.selected as i64;
        let turned = (state.incoming != self.turn.serial).then_some(self.turn.by);
        state.incoming = self.turn.serial;
        let aim = match (state.aim, turned) {
            (Some(aim), Some(by)) if (aim + by).rem_euclid(count) == selected => aim + by,
            (Some(aim), _) if aim.rem_euclid(count) == selected => aim,
            (Some(aim), _) => aim + nearest(selected - aim.rem_euclid(count), count),
            (None, _) => selected,
        };
        state.aim = Some(aim);
        aim
    }

    fn turn_to(&self, place: i64, state: &mut GalleryState, ctx: &mut EventCtx<'_>) {
        state.aim = Some(place);
        let index = place.rem_euclid(self.tiles.len() as i64) as usize;
        if index == self.selected {
            ctx.request_paint();
        } else {
            ctx.call(&self.on_select, (index,));
        }
    }

    fn within(&self, at: f32) -> impl Iterator<Item = Spot> + '_ {
        let count = self.tiles.len() as i64;
        let first = (at - REACH).ceil() as i64;
        let last = (at + REACH).floor() as i64;
        (first..=last).map(move |place| Spot {
            place,
            index: place.rem_euclid(count) as usize,
        })
    }

    fn draw(
        &self,
        index: usize,
        rect: Rect,
        selected: bool,
        playing: Option<&VideoController>,
        ctx: &mut PaintCtx<'_>,
    ) {
        let Some(tile) = self.tiles.get(index) else {
            return;
        };
        let palette = palette();
        let radius = RADIUS * (rect.height() / TILE.height);
        let shape = RoundedRect::new(rect, CornerRadii::uniform(radius));

        ctx.draw_quad(Quad::new(rect).radius(radius).fill(palette.surface_variant));
        let source = source(&tile.preview, selected.then_some(playing).flatten());
        if source.as_ref().is_none_or(|source| !has_pixels(source)) {
            waiting(&tile.preview, rect, ctx);
        }
        if let Some(source) = source {
            let content = fitted(rect, &source);
            ctx.clipped(shape, |ctx| ctx.draw_image(content, source));
        }
        if selected {
            ctx.draw_quad(Quad::new(rect).radius(radius).stroke(RING, palette.accent));
        }
        if tile.in_use {
            badge(rect, ctx);
        }
        write(tile, rect, selected, ctx);
    }
}

fn write(tile: &Tile, rect: Rect, selected: bool, ctx: &mut PaintCtx<'_>) {
    let palette = palette();
    let color = match selected {
        true => palette.text,
        false => palette.text_muted,
    };
    let run = TextRun::new(tile.label.as_str(), LABEL_SIZE)
        .no_wrap()
        .align(TextAlign::Center);
    let line = Rect::new(
        rect.center().x - TILE.width / 2.0,
        rect.max_y() + LABEL_GAP,
        TILE.width,
        LABEL_HEIGHT,
    );
    ctx.draw_text(line, run, color);
}

fn step(delta: ScrollDelta) -> Option<i64> {
    let travel = delta.offset().x + delta.offset().y;
    match travel {
        travel if travel > 0.0 => Some(1),
        travel if travel < 0.0 => Some(-1),
        _ => None,
    }
}

fn offset(spot: Spot, at: f32) -> f32 {
    spot.place as f32 - at
}

fn nearest(delta: i64, count: i64) -> i64 {
    let ahead = delta.rem_euclid(count);
    match ahead * 2 > count {
        true => ahead - count,
        false => ahead,
    }
}

fn place(axis: Point, offset: f32) -> Rect {
    let scale = SMALLEST + (1.0 - SMALLEST) * ease(1.0 - (offset.abs() / REACH).clamp(0.0, 1.0));
    Rect::from_center(
        Point::new(axis.x + offset * STRIDE, axis.y),
        Size::new(TILE.width * scale, TILE.height * scale),
    )
}

fn fade(offset: f32) -> f32 {
    let over = (offset.abs() - (REACH - FADE)).max(0.0);
    1.0 - ease((over / FADE).clamp(0.0, 1.0))
}

fn ease(t: f32) -> f32 {
    Easing::Standard.transform(t)
}

fn source(preview: &Preview, playing: Option<&VideoController>) -> Option<ImageSource> {
    match preview {
        Preview::Still(source) => source.clone(),
        Preview::Motion(_) => playing.map(ImageSource::from),
    }
}

fn has_pixels(source: &ImageSource) -> bool {
    source.natural_size().is_some()
}

fn fitted(rect: Rect, source: &ImageSource) -> Rect {
    ImageFit::Cover.place(rect, source.natural_size().unwrap_or(Size::ZERO))
}

fn waiting(preview: &Preview, rect: Rect, ctx: &mut PaintCtx<'_>) {
    let symbol = match preview {
        Preview::Still(_) => MaterialSymbol::IMAGE,
        Preview::Motion(_) => MaterialSymbol::MOVIE,
    };
    let size = (rect.height() / 3.0).min(32.0);
    ctx.draw_icon(
        Rect::from_center(rect.center(), Size::new(size, size)),
        IconGlyph::from(symbol).spec(size),
        palette().text_muted,
    );
}

fn badge(rect: Rect, ctx: &mut PaintCtx<'_>) {
    let palette = palette();
    let centre = Point::new(
        rect.max_x() - BADGE_INSET - BADGE / 2.0,
        rect.min_y() + BADGE_INSET + BADGE / 2.0,
    );
    ctx.draw_circle(centre, BADGE + 6.0, palette.accent);
    ctx.draw_icon(
        Rect::from_center(centre, Size::new(BADGE, BADGE)),
        IconGlyph::from(MaterialSymbol::CHECK).spec(BADGE),
        palette.on_accent,
    );
}

fn at_point(placed: &[(Spot, Rect)], point: Point) -> Option<Spot> {
    placed
        .iter()
        .rev()
        .find(|(_, rect)| rect.contains(point))
        .map(|(spot, _)| *spot)
}

impl View for Gallery {
    fn build(self) -> Element {
        Element::new(self)
    }
}

impl std::fmt::Debug for Gallery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gallery")
            .field("tiles", &self.tiles.len())
            .field("selected", &self.selected)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for Tile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tile")
            .field("label", &self.label)
            .finish_non_exhaustive()
    }
}
