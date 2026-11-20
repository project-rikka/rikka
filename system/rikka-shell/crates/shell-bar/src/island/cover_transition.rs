use rikka_types::{CornerRadii, Point, Rect, Size};
use rikka_ui::{Curve, IconGlyph, Length, MaterialSymbol, PaintCtx, Quad, Sprite, Styled, motion};
use rikka_widgets::{Spacer, spacer};
use shell_feeds::{BLANK_SYMBOL, Cover};

const SYMBOL_STEP: f32 = 2.0;

const PLACE: &str = "island.flight.place";

const ROUNDING: &str = "island.flight.rounding";

const SHOWN: &str = "island.flight.shown";
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct CoverFrame {
    at: Point,
    side: f32,
    rounding: f32,
}

impl CoverFrame {
    #[must_use]
    pub(crate) const fn new(side: f32, rounding: f32) -> CoverFrame {
        CoverFrame {
            at: Point::ZERO,
            side,
            rounding,
        }
    }

    #[must_use]
    pub(crate) fn inset(self, by: Point) -> CoverFrame {
        CoverFrame {
            at: self.at + by,
            ..self
        }
    }

    #[must_use]
    pub(crate) fn seat(self) -> Spacer {
        spacer()
            .size(Length::px(self.side), Length::px(self.side))
            .grow(0.0)
            .shrink(0.0)
    }

    fn placed_in(self, owner: Rect) -> Rect {
        Rect::new(
            owner.x() + self.at.x,
            owner.y() + self.at.y,
            self.side,
            self.side,
        )
    }
}

#[derive(Clone, Debug)]
pub(super) struct CoverTarget {
    frame: CoverFrame,
    cover: Cover,
}

impl CoverTarget {
    #[must_use]
    pub(crate) fn new(frame: CoverFrame, cover: Cover) -> CoverTarget {
        CoverTarget { frame, cover }
    }

    #[must_use]
    pub(crate) fn inset(self, by: Point) -> CoverTarget {
        CoverTarget {
            frame: self.frame.inset(by),
            ..self
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct CoverTransition {
    rest: Option<CoverTarget>,
    open: Option<CoverTarget>,
    seated: bool,
}

#[derive(Debug, Default)]
pub(crate) struct CoverTransitionState {
    cover: Option<Cover>,
}

impl CoverTransition {
    #[must_use]
    pub(crate) fn new(
        rest: Option<CoverTarget>,
        open: Option<CoverTarget>,
        seated: bool,
    ) -> CoverTransition {
        CoverTransition { rest, open, seated }
    }

    pub(crate) fn paint(
        &self,
        flown: &mut CoverTransitionState,
        ctx: &mut PaintCtx<'_>,
        face: Rect,
        card: Option<Rect>,
        curve: Curve,
    ) {
        let open_card = card.and_then(|card| {
            self.open
                .as_ref()
                .map(|cover_target| (cover_target.frame.placed_in(card), cover_target))
        });
        let resting = self
            .rest
            .as_ref()
            .map(|cover_target| (cover_target.frame.placed_in(face), cover_target));
        let (place, rounding, shown, cover) = match (open_card, resting) {
            (Some((place, cover_target)), _) => (
                place,
                cover_target.frame.rounding,
                1.0,
                Some(&cover_target.cover),
            ),
            (None, Some((place, cover_target))) => (
                place,
                cover_target.frame.rounding,
                if self.seated { 1.0 } else { 0.0 },
                Some(&cover_target.cover),
            ),
            (None, None) => (Rect::from_center(face.center(), Size::ZERO), 0.0, 0.0, None),
        };
        if let Some(cover) = cover {
            flown.cover = Some(cover.clone());
        }
        let place = ctx.animate_curve(PLACE, place, curve);
        let rounding = ctx.animate_curve(ROUNDING, rounding, curve);
        let shown = ctx
            .animate_curve(SHOWN, shown, motion::FILL)
            .clamp(0.0, 1.0);
        if shown <= f32::EPSILON || place.width() < 1.0 || place.height() < 1.0 {
            if cover.is_none() {
                flown.cover = None;
            }
            return;
        }
        let Some(cover) = flown.cover.as_ref() else {
            return;
        };
        let radii = CornerRadii::uniform(rounding.clamp(0.0, place.width() / 2.0));
        let place = place.translate(ctx.bounds().origin);
        ctx.topmost(|ctx| ctx.with_opacity(shown, |ctx| draw(ctx, cover, place, radii)));
    }
}

fn draw(ctx: &mut PaintCtx<'_>, cover: &Cover, place: Rect, radii: CornerRadii) {
    match cover {
        Cover::Picture(source) => ctx.draw_sprite(Sprite::new(place, source.clone()).radius(radii)),
        Cover::Blank => {
            let palette = ctx.palette();
            ctx.draw_quad(Quad::new(place).fill(palette.surface_variant).radius(radii));
            let size = (place.width() * BLANK_SYMBOL / SYMBOL_STEP).round() * SYMBOL_STEP;
            if size >= SYMBOL_STEP {
                let symbol = IconGlyph::from(MaterialSymbol::MUSIC_NOTE).spec(size);
                ctx.draw_icon(
                    Rect::from_center(place.center(), Size::new(size, size)),
                    symbol,
                    palette.text_muted,
                );
            }
        }
    }
}
