use std::sync::Arc;
use std::time::Duration;

use overview_layout::{Spacing, arrange};
use rikka_theme::Theme;
use rikka_types::{CornerRadii, Point, Rect, RoundedRect, Size, WindowView};
use rikka_ui::{Curve, PaintCtx, Quad, Spring, is_exiting, theme};
use shell_wallpaper::stage::{CARD_RADIUS, RING_WIDTH, THUMB_RADIUS, draw_fitted_image, lerp_rect};

use super::frame::{COLUMN_SPACING, Frame, ROW_SPACING, from_unit, unit_of};
use super::input::Spot;
use super::strip::SETTLE;
use super::{Deck, DeckState, Hit, Hits, Pane, Space};

const LIFT: f32 = 0.025;
const SHRUNK: f32 = 0.88;
const HELD_MOST: f32 = 320.0;
const LOST: Duration = Duration::from_millis(600);

const GLIDE: Curve = Curve::Spring(Spring::DEFAULT_SPATIAL);
const LIFTING: Curve = Curve::Spring(Spring::new(1.0, 900.0));

struct Painting<'a> {
    frame: Frame,
    spread: f32,
    focus: f32,
    theme: &'a Theme,
    target: Option<usize>,
}

impl Deck {
    pub(super) fn draw(&self, state: &mut DeckState, ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        if bounds.is_empty() {
            return;
        }
        let board = &self.board;
        let theme = theme().clone();
        board.swipes.watch(ctx.handle());
        let swiped = board.swipes.now();
        self.init_scroll(state, &swiped);
        let spread = self.spread(state, &swiped, ctx);
        let frame = Frame::new(bounds, board.backdrop.taskbar, board.spaces.len());
        let focus = self.focus(state, &swiped, spread, frame.pitch(), ctx);
        state.spread = spread;
        board.frost.set(&board.output, spread);
        state.settled = spread >= 1.0 && !is_exiting();
        state.hits = Hits {
            pitch: frame.pitch(),
            ..Hits::default()
        };
        let painting = Painting {
            frame,
            spread,
            focus,
            theme: &theme,
            target: self.drop_target(state),
        };
        ctx.clipped(frame.stage, |ctx| {
            self.draw_thumbs(state, &painting, ctx);
            let cards: Vec<(usize, Rect)> = (0..board.spaces.len())
                .filter_map(|index| self.draw_space(index, state, &painting, ctx))
                .collect();
            board.cards.lay(cards);
            self.draw_held(state, ctx);
        });
        self.settle_homes(state);
    }

    fn draw_thumbs(&self, state: &mut DeckState, painting: &Painting<'_>, ctx: &mut PaintCtx<'_>) {
        let spread = painting.spread;
        if spread <= 0.0 {
            return;
        }
        let palette = painting.theme.palette();
        let dragged = state.drag.as_ref().map(|drag| Arc::clone(&drag.id));
        for (index, space) in self.board.spaces.iter().enumerate() {
            let thumb = painting.frame.thumb(index, spread);
            let shape = RoundedRect::new(thumb, CornerRadii::uniform(THUMB_RADIUS));
            let current = index == self.board.showing;
            let aimed = painting.target == Some(space.number);
            let hovered = state.drag.is_none() && state.over == Some(Spot::Thumb(space.number));
            ctx.with_opacity(spread, |ctx| {
                ctx.draw_quad(
                    Quad::new(thumb)
                        .radius(THUMB_RADIUS)
                        .fill(palette.background),
                );
                ctx.clipped(shape, |ctx| {
                    if let Some(picture) = &self.board.backdrop.wallpaper {
                        draw_fitted_image(ctx, thumb, picture);
                    }
                    if aimed {
                        ctx.draw_quad(Quad::new(thumb).fill(palette.accent.with_alpha(0.28)));
                    }
                    let shown = space
                        .panes
                        .iter()
                        .filter(|pane| !pane.minimized && dragged.as_ref() != Some(&pane.id));
                    for pane in shown {
                        let rect = painting.frame.placed_within(pane.real, thumb);
                        ctx.draw_window(WindowView::new(Arc::clone(&pane.id), rect));
                    }
                });
                let edge = if current || aimed {
                    Some(palette.accent)
                } else if hovered {
                    Some(palette.text_muted)
                } else {
                    None
                };
                if let Some(edge) = edge {
                    ctx.draw_quad(
                        Quad::new(thumb.inflate(RING_WIDTH))
                            .radius(THUMB_RADIUS + RING_WIDTH)
                            .stroke(RING_WIDTH, edge),
                    );
                }
            });
            state.hits.thumbs.push((space.number, thumb));
        }
    }

    fn draw_space(
        &self,
        index: usize,
        state: &mut DeckState,
        painting: &Painting<'_>,
        ctx: &mut PaintCtx<'_>,
    ) -> Option<(usize, Rect)> {
        let space = self.board.spaces.get(index)?;
        let frame = &painting.frame;
        let spread = painting.spread;
        let card = frame.card(index as f32 - painting.focus, spread);
        let current = index == self.board.showing;
        let shown = card.intersects(&frame.stage.rect);
        if shown {
            self.draw_card(space, card, current, state, painting, ctx);
        }

        let settled_card = frame.settled_card();
        let reals = space
            .panes
            .iter()
            .map(|pane| frame.placed_within(pane.real, settled_card));
        let mut laid = state.laid.remove(&space.number).unwrap_or_default();
        laid.settle(reals, frame.windows_area(), settled_card.height());
        for (pane, slot) in space.panes.iter().zip(laid.slots.iter().copied()) {
            let unit = self.glide(
                state,
                pane,
                space.number,
                unit_of(slot, settled_card),
                card,
                ctx,
            );
            let is_dragged = state.drag.as_ref().is_some_and(|drag| drag.id == pane.id);
            if is_dragged || !shown {
                continue;
            }
            let rect = match pinned(state, pane, space.number) {
                Some(rect) => rect,
                None => self.resting(pane, unit, card, painting),
            };
            let placed = Placed {
                pane,
                number: space.number,
                rect,
                current,
            };
            self.draw_pane(placed, state, painting, ctx);
        }
        state.laid.insert(space.number, laid);
        Some((space.number, card))
    }

    fn draw_card(
        &self,
        space: &Space,
        card: Rect,
        current: bool,
        state: &mut DeckState,
        painting: &Painting<'_>,
        ctx: &mut PaintCtx<'_>,
    ) {
        let palette = painting.theme.palette();
        let radius = CARD_RADIUS * painting.spread;
        let shape = RoundedRect::new(card, CornerRadii::uniform(radius));
        if let Some(picture) = &self.board.backdrop.wallpaper {
            ctx.clipped(shape, |ctx| draw_fitted_image(ctx, card, picture));
        }
        let aimed = painting.target == Some(space.number);
        let hovered = state.settled
            && state.drag.is_none()
            && !current
            && state.over == Some(Spot::Card(space.number));
        let edge = if aimed {
            Some(palette.accent)
        } else if hovered {
            Some(palette.text_muted)
        } else {
            None
        };
        if let Some(edge) = edge {
            ctx.draw_quad(
                Quad::new(card.inflate(RING_WIDTH))
                    .radius(radius + RING_WIDTH)
                    .stroke(RING_WIDTH, edge),
            );
        }
        state.hits.cards.push((space.number, card));
    }

    fn resting(&self, pane: &Pane, unit: Rect, card: Rect, painting: &Painting<'_>) -> Rect {
        let rest = from_unit(unit, card);
        let start = match pane.minimized {
            true => scaled(rest, SHRUNK),
            false => painting.frame.placed_within(pane.real, card),
        };
        lerp_rect(start, rest, painting.spread)
    }

    fn draw_pane(
        &self,
        placed: Placed<'_>,
        state: &mut DeckState,
        painting: &Painting<'_>,
        ctx: &mut PaintCtx<'_>,
    ) {
        let Placed {
            pane,
            number,
            rect,
            current,
        } = placed;
        let lifted = self.lifted(state) == Some(&pane.id);
        let lift = ctx.animate_curve(("lift", &*pane.id), if lifted { 1.0 } else { 0.0 }, LIFTING);
        let rect = scaled(rect, 1.0 + LIFT * lift);
        let opacity = match pane.minimized {
            true => painting.spread,
            false => 1.0,
        };
        ctx.draw_window(WindowView::new(Arc::clone(&pane.id), rect).opacity(opacity));
        if current {
            state.hits.windows.push(Hit {
                id: Arc::clone(&pane.id),
                number,
                rect,
            });
        }
    }

    fn glide(
        &self,
        state: &mut DeckState,
        pane: &Pane,
        number: usize,
        unit: Rect,
        card: Rect,
        ctx: &mut PaintCtx<'_>,
    ) -> Rect {
        let key = ("slot", &*pane.id);
        let moved = state.homes.get(&pane.id).is_some_and(|was| *was != number);
        let landed = state.dropped.as_ref().and_then(|dropped| {
            (dropped.id == pane.id && dropped.to.is_none_or(|to| to == number))
                .then_some(dropped.rect)
        });
        match landed {
            Some(rect) => {
                ctx.jump(key, unit_of(rect, card));
                state.dropped = None;
            }
            None if moved => ctx.jump(key, unit),
            None => {}
        }
        ctx.animate_curve(key, unit, GLIDE)
    }

    fn draw_held(&self, state: &mut DeckState, ctx: &mut PaintCtx<'_>) {
        let Some(drag) = &mut state.drag else {
            return;
        };
        let key = ("held", &*drag.id);
        if std::mem::take(&mut drag.fresh) {
            ctx.jump(key, drag.from.size);
        }
        let size = ctx.animate_curve(key, held_size(drag.from.size), SETTLE);
        let rect = Rect::from_origin_size(
            Point::new(
                drag.at.x - drag.grab.x * size.width,
                drag.at.y - drag.grab.y * size.height,
            ),
            size,
        );
        drag.rect = rect;
        ctx.draw_window(WindowView::new(Arc::clone(&drag.id), rect));
    }

    fn settle_homes(&self, state: &mut DeckState) {
        let spaces = &self.board.spaces;
        state
            .laid
            .retain(|number, _| spaces.iter().any(|space| space.number == *number));
        let homes = spaces
            .iter()
            .flat_map(|space| space.panes.iter().map(move |pane| (&pane.id, space.number)));
        let unchanged = homes.clone().count() == state.homes.len()
            && homes
                .clone()
                .all(|(id, number)| state.homes.get(id) == Some(&number));
        if !unchanged {
            state.homes = homes.map(|(id, number)| (Arc::clone(id), number)).collect();
        }
        if state.dropped.as_ref().is_some_and(|dropped| {
            dropped.at.elapsed() > LOST || !state.homes.contains_key(&dropped.id)
        }) {
            state.dropped = None;
        }
    }

    fn drop_target(&self, state: &DeckState) -> Option<usize> {
        let drag = state.drag.as_ref()?;
        match state.over {
            Some(Spot::Card(number) | Spot::Thumb(number)) if number != drag.number => Some(number),
            _ => None,
        }
    }
}

fn pinned(state: &DeckState, pane: &Pane, number: usize) -> Option<Rect> {
    let dropped = state.dropped.as_ref()?;
    (dropped.id == pane.id && dropped.to.is_some_and(|to| to != number)).then_some(dropped.rect)
}

fn held_size(size: Size) -> Size {
    let largest = size.width.max(size.height).max(1.0);
    let shrink = (HELD_MOST / largest).min(1.0);
    Size::new(size.width * shrink, size.height * shrink)
}

fn scaled(rect: Rect, by: f32) -> Rect {
    Rect::from_center(
        rect.center(),
        Size::new(rect.width() * by, rect.height() * by),
    )
}

struct Placed<'p> {
    pane: &'p Pane,
    number: usize,
    rect: Rect,
    current: bool,
}

#[derive(Debug, Default)]
pub(super) struct DeckLayout {
    reals: Vec<Rect>,
    area: Rect,
    tall: f32,
    slots: Vec<Rect>,
}

impl DeckLayout {
    fn settle(&mut self, reals: impl Iterator<Item = Rect> + Clone, area: Rect, tall: f32) {
        if self.reals.iter().copied().eq(reals.clone()) && self.area == area && self.tall == tall {
            return;
        }
        self.reals.clear();
        self.reals.extend(reals);
        let spacing = Spacing {
            row: ROW_SPACING,
            column: COLUMN_SPACING,
        };
        self.slots = arrange(&self.reals, area, spacing, tall);
        self.area = area;
        self.tall = tall;
    }
}
