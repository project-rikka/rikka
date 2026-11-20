use rikka_compositor_geometry::{Physical, Point, Rect, Size};
use rikka_renderer::{Blur, Joined, Spread};
use rikka_types::{Lens, Rect as SceneRect, RoundedRect};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Piece {
    pub(crate) shape: RoundedRect,
    pub(crate) hollow: Option<RoundedRect>,
    pub(crate) joined: Option<Joined>,
}

impl Piece {
    pub(crate) fn lone(shape: RoundedRect) -> Piece {
        Piece {
            shape,
            hollow: None,
            joined: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Pane {
    pub(crate) key: u64,
    pub(crate) piece: Piece,
    pub(crate) radius: f32,
    pub(crate) lens: Option<Lens>,
    pub(crate) alpha: f32,
    spread: Spread,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Look {
    pub(super) piece: Piece,
    pub(super) radius: f32,
    pub(super) lens: Option<Lens>,
}

impl Pane {
    pub(crate) fn new(key: u64, piece: Piece, radius: f32, lens: Option<Lens>, alpha: f32) -> Self {
        Self {
            key,
            piece,
            radius,
            lens,
            alpha,
            spread: Spread::new(piece.shape, &Blur::with_radius(radius), lens, piece.joined),
        }
    }

    pub(crate) fn cut_like(&self, piece: Piece, radius: f32, lens: Option<Lens>) -> bool {
        self.piece == piece && self.radius == radius && self.lens == lens
    }

    pub(crate) fn faded(self, alpha: f32) -> Self {
        Self { alpha, ..self }
    }
    pub(crate) fn reads(&self) -> i32 {
        self.spread.reads().ceil() as i32
    }

    pub(crate) fn geometry(&self) -> Rect<i32, Physical> {
        let rect = self.piece.shape.rect;
        let left = rect.min_x().floor() as i32;
        let top = rect.min_y().floor() as i32;
        Rect::new(
            Point::new(left, top),
            Size::new(
                rect.max_x().ceil() as i32 - left,
                rect.max_y().ceil() as i32 - top,
            ),
        )
    }

    pub(crate) fn watched(&self) -> Rect<i32, Physical> {
        self.geometry().grown(self.reads())
    }

    pub(crate) fn touched_by(
        &self,
        changed: Rect<i32, Physical>,
        stale: &mut Vec<Rect<i32, Physical>>,
    ) {
        self.touched_by_scene(changed.into(), stale);
    }

    pub(super) fn reshaped_from(&self, was: Look, stale: &mut Vec<Rect<i32, Physical>>) -> bool {
        let (now, then) = (self.piece, was.piece);
        let plain = now.joined.is_none() && then.joined.is_none() && now.hollow == then.hollow;
        if !plain || was.radius != self.radius || was.lens != self.lens {
            return false;
        }
        let Some(corners) = now.shape.changed_corners(&then.shape) else {
            return false;
        };
        for corner in corners {
            self.touched_by_scene(corner, stale);
        }
        true
    }

    fn touched_by_scene(&self, changed: SceneRect, stale: &mut Vec<Rect<i32, Physical>>) {
        let mut touched = Vec::new();
        self.spread.stale_regions(changed, &mut touched);
        stale.extend(touched.iter().copied().map(Rect::covering));
    }

    pub(super) fn look(&self) -> Look {
        Look {
            piece: self.piece,
            radius: self.radius,
            lens: self.lens,
        }
    }
}
