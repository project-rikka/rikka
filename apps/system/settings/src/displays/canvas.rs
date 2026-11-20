use rikka_app::prelude::*;
use settings_displays::{Bounds, arrangement};

const MARGIN: f32 = 16.0;
pub(crate) const HEIGHT: f32 = 240.0;
const RADIUS: f32 = Corner::Item.radius();
const CHARACTER_WIDTH: f32 = 0.55;

#[derive(Clone, Debug)]
pub(crate) struct Tile {
    pub(crate) bounds: Bounds,
    pub(crate) number: usize,
    pub(crate) caption: String,
    pub(crate) selected: bool,
    pub(crate) enabled: bool,
}

#[derive(Debug, Default)]
pub(crate) struct Dragging {
    drag: Option<Drag>,
}

#[derive(Clone, Copy, Debug)]
struct Drag {
    index: usize,
    grab: Point,
    at: Point,
}

pub(crate) struct Canvas {
    tiles: Vec<Tile>,
    on_select: Handler<(usize,)>,
    on_move: Handler<(usize, (i32, i32))>,
}

impl Canvas {
    pub(crate) fn new<S, M>(
        tiles: Vec<Tile>,
        on_select: impl IntoHandler<(usize,), (), S>,
        on_move: impl IntoHandler<(usize, (i32, i32)), (), M>,
    ) -> Self {
        Self {
            tiles,
            on_select: on_select.into_handler(),
            on_move: on_move.into_handler(),
        }
    }

    fn fit(&self, bounds: Rect) -> Fit {
        let union = arrangement::union(self.tiles.iter().map(|tile| tile.bounds));
        let room = bounds.inflate(-MARGIN);
        let scale = (room.size.width / union.width.max(1) as f32)
            .min(room.size.height / union.height.max(1) as f32)
            .min(1.0);
        Fit {
            scale,
            origin: Point::new(
                room.origin.x + (room.size.width - union.width as f32 * scale) / 2.0
                    - union.x as f32 * scale,
                room.origin.y + (room.size.height - union.height as f32 * scale) / 2.0
                    - union.y as f32 * scale,
            ),
        }
    }

    fn dragged_to(&self, drag: Drag, fit: Fit) -> Option<Bounds> {
        let tile = self.tiles.get(drag.index)?;
        let corner = Point::new(drag.at.x - drag.grab.x, drag.at.y - drag.grab.y);
        Some(Bounds {
            x: fit.to_arrangement_x(corner.x),
            y: fit.to_arrangement_y(corner.y),
            ..tile.bounds
        })
    }

    fn snap(&self, index: usize, dropped: Bounds) -> (i32, i32) {
        let others: Vec<Bounds> = self
            .tiles
            .iter()
            .enumerate()
            .filter(|(other, _)| *other != index)
            .map(|(_, tile)| tile.bounds)
            .collect();
        arrangement::snap(dropped, &others)
    }
}

#[derive(Clone, Copy, Debug)]
struct Fit {
    scale: f32,
    origin: Point,
}

impl Fit {
    fn to_canvas(self, bounds: Bounds) -> Rect {
        Rect::new(
            self.origin.x + bounds.x as f32 * self.scale,
            self.origin.y + bounds.y as f32 * self.scale,
            bounds.width as f32 * self.scale,
            bounds.height as f32 * self.scale,
        )
    }

    fn to_arrangement_x(self, x: f32) -> i32 {
        ((x - self.origin.x) / self.scale).round() as i32
    }

    fn to_arrangement_y(self, y: f32) -> i32 {
        ((y - self.origin.y) / self.scale).round() as i32
    }
}

impl Widget for Canvas {
    type State = Dragging;

    fn paint(&self, state: &mut Self::State, ctx: &mut PaintCtx<'_>) {
        let palette = palette();
        let bounds = ctx.bounds();
        let fit = self.fit(bounds);

        ctx.draw_quad(
            Quad::new(bounds)
                .fill(palette.surface)
                .radius(Corner::Panel),
        );

        let dragging = state.drag.and_then(|drag| {
            self.dragged_to(drag, fit)
                .map(|bounds| (drag.index, bounds))
        });

        for (index, tile) in self.tiles.iter().enumerate() {
            let placed = match dragging {
                Some((drag, bounds)) if drag == index => bounds,
                _ => tile.bounds,
            };
            self.paint_tile(ctx, tile, fit.to_canvas(placed));
        }
    }

    fn event(&self, state: &mut Self::State, event: &Event, ctx: &mut EventCtx<'_>) {
        let fit = self.fit(ctx.bounds());
        match *event {
            Event::PointerPressed {
                position,
                button: PointerButton::Primary,
                ..
            } => {
                let Some((index, tile)) = self
                    .tiles
                    .iter()
                    .enumerate()
                    .rev()
                    .find(|(_, tile)| fit.to_canvas(tile.bounds).contains(position))
                else {
                    return;
                };
                let corner = fit.to_canvas(tile.bounds).origin;
                state.drag = Some(Drag {
                    index,
                    grab: Point::new(position.x - corner.x, position.y - corner.y),
                    at: position,
                });
                ctx.capture_pointer();
                ctx.set_handled();
                ctx.call(&self.on_select, (index,));
            }
            Event::PointerMoved(position) => {
                let Some(drag) = &mut state.drag else {
                    return;
                };
                drag.at = position;
                ctx.request_paint();
                ctx.set_handled();
            }
            Event::PointerReleased {
                position,
                button: PointerButton::Primary,
                ..
            } => {
                let Some(mut drag) = state.drag.take() else {
                    return;
                };
                drag.at = position;
                ctx.release_pointer();
                ctx.set_handled();
                ctx.request_paint();
                let Some(dropped) = self.dragged_to(drag, fit) else {
                    return;
                };
                let snapped = self.snap(drag.index, dropped);
                ctx.call(&self.on_move, (drag.index, snapped));
            }
            _ => {}
        }
    }
}

impl Canvas {
    fn paint_tile(&self, ctx: &mut PaintCtx<'_>, tile: &Tile, rect: Rect) {
        let palette = palette();
        let (fill, ink) = match (tile.enabled, tile.selected) {
            (true, true) => (palette.selection, palette.on_selection),
            (true, false) => (palette.surface_variant, palette.text),
            (false, _) => (palette.background, palette.text_muted),
        };
        let edge = if tile.selected {
            palette.accent
        } else {
            palette.outline
        };

        ctx.draw_quad(
            Quad::new(rect)
                .fill(fill)
                .radius(RADIUS)
                .stroke(if tile.selected { 2.0 } else { 1.0 }, edge),
        );
        ctx.clipped(RoundedRect::new(rect, RADIUS), |ctx| {
            let size = (rect.size.min_side() * 0.45).clamp(11.0, 48.0);
            let number = tile.number.to_string();
            centred(ctx, rect, rect.center().y - size * 0.62, &number, size, ink);

            let caption = (rect.size.height > size * 2.4).then_some(&tile.caption);
            if let Some(caption) = caption {
                let small = (size * 0.32).clamp(9.0, 13.0);
                centred(
                    ctx,
                    rect,
                    rect.center().y + size * 0.5,
                    caption,
                    small,
                    palette.text_muted,
                );
            }
        });
    }
}

fn centred(ctx: &mut PaintCtx<'_>, within: Rect, top: f32, text: &str, size: f32, ink: Color) {
    let width = text.chars().count() as f32 * size * CHARACTER_WIDTH;
    let left = within.center().x - width / 2.0;
    ctx.draw_text(
        Rect::new(left.max(within.min_x()), top, width.max(1.0), size * 1.4),
        TextRun::new(text, size),
        ink,
    );
}
