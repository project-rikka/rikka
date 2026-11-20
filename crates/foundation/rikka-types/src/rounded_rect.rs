use crate::{CornerRadii, Point, Rect};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RoundedRect {
    pub rect: Rect,
    pub radii: CornerRadii,
}

impl RoundedRect {
    pub const CORNER_REACH: f32 = 1.6;

    pub const fn rect(rect: Rect) -> Self {
        Self {
            rect,
            radii: CornerRadii::ZERO,
        }
    }

    pub fn new(rect: Rect, radii: impl Into<CornerRadii>) -> Self {
        Self {
            rect,
            radii: radii.into(),
        }
    }

    pub fn circle(center: Point, radius: f32) -> Self {
        Self::new(Rect::from_circle(center, radius), radius)
    }

    pub fn intersect(&self, other: impl Into<RoundedRect>) -> Option<Self> {
        let other = other.into();
        let rect = self.rect.intersection(&other.rect)?;
        let ours = self.rounding_within(&rect);
        let theirs = other.rounding_within(&rect);
        let radii = CornerRadii::new(
            ours.top_left.max(theirs.top_left),
            ours.top_right.max(theirs.top_right),
            ours.bottom_right.max(theirs.bottom_right),
            ours.bottom_left.max(theirs.bottom_left),
        );
        Some(Self {
            rect,
            radii: radii.clamp_for_size(rect.size),
        })
    }

    fn rounding_within(&self, inner: &Rect) -> CornerRadii {
        let radii = self.radii.clamp_for_size(self.rect.size);
        let left = inner.min_x() - self.rect.min_x();
        let top = inner.min_y() - self.rect.min_y();
        let right = self.rect.max_x() - inner.max_x();
        let bottom = self.rect.max_y() - inner.max_y();
        let reached = |radius: f32, across: f32, down: f32| (radius - across.max(down)).max(0.0);
        CornerRadii::new(
            reached(radii.top_left, left, top),
            reached(radii.top_right, right, top),
            reached(radii.bottom_right, right, bottom),
            reached(radii.bottom_left, left, bottom),
        )
    }

    pub fn scale(&self, factor: f32) -> Self {
        Self {
            rect: self.rect.scale(factor),
            radii: self.radii * factor,
        }
    }

    pub fn changed_corners(&self, from: &Self) -> Option<impl Iterator<Item = Rect> + use<>> {
        if self.rect != from.rect {
            return None;
        }
        let ours = self.radii.clamp_for_size(self.rect.size).to_array();
        let theirs = from.radii.clamp_for_size(from.rect.size).to_array();
        let shape = *self;
        Some((0..4).filter_map(move |corner| {
            let (now, was) = (ours[corner], theirs[corner]);
            (now != was).then(|| shape.corner_square(corner, shape.reach_of(now.max(was))))
        }))
    }

    pub fn rim(&self, width: f32) -> impl Iterator<Item = Rect> + use<> {
        let rect = self.rect;
        let band = (width.max(0.0) + EDGE).min(rect.width()).min(rect.height());
        let sides = [
            Rect::new(rect.x(), rect.y(), rect.width(), band),
            Rect::new(rect.x(), rect.max_y() - band, rect.width(), band),
            Rect::new(rect.x(), rect.y(), band, rect.height()),
            Rect::new(rect.max_x() - band, rect.y(), band, rect.height()),
        ];
        let radii = self.radii.clamp_for_size(rect.size).to_array();
        let shape = *self;
        let corners = (0..4)
            .filter(move |&corner| radii[corner] > 0.0)
            .map(move |corner| shape.corner_square(corner, shape.reach_of(radii[corner])));
        sides.into_iter().chain(corners)
    }

    pub fn cores(&self) -> [Rect; 2] {
        let rect = self.rect;
        let limit = rect.size.min_side() * 0.5;
        let [tl, tr, br, bl] = self
            .radii
            .clamp_for_size(rect.size)
            .to_array()
            .map(|radius| (radius * Self::CORNER_REACH).min(limit));
        let (top, bottom, left, right) = (tl.max(tr), br.max(bl), tl.max(bl), tr.max(br));
        [
            Rect::new(
                rect.x(),
                rect.y() + top,
                rect.width(),
                rect.height() - top - bottom,
            ),
            Rect::new(
                rect.x() + left,
                rect.y(),
                rect.width() - left - right,
                rect.height(),
            ),
        ]
    }

    fn reach_of(&self, radius: f32) -> f32 {
        let limit = self.rect.size.min_side() * 0.5;
        (radius * Self::CORNER_REACH).min(limit) + EDGE
    }

    fn corner_square(&self, corner: usize, side: f32) -> Rect {
        let rect = self.rect;
        let side = side.min(rect.width()).min(rect.height());
        let left = match corner {
            0 | 3 => rect.x(),
            _ => rect.max_x() - side,
        };
        let top = match corner {
            0 | 1 => rect.y(),
            _ => rect.max_y() - side,
        };
        Rect::new(left, top, side, side)
    }
}

const EDGE: f32 = 1.0;

impl From<Rect> for RoundedRect {
    fn from(rect: Rect) -> Self {
        Self::rect(rect)
    }
}
