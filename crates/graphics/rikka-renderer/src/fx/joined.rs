use rikka_types::{CornerRadii, Point, Rect, RoundedRect};

const TOUCH: f32 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    Below,
    Above,
    Right,
    Left,
}

impl Side {
    const ALL: [Side; 4] = [Side::Below, Side::Above, Side::Right, Side::Left];

    fn code(self) -> f32 {
        match self {
            Side::Below => 0.0,
            Side::Above => 1.0,
            Side::Right => 2.0,
            Side::Left => 3.0,
        }
    }

    fn canon(self, p: Point) -> Point {
        match self {
            Side::Below => p,
            Side::Above => Point::new(p.x, -p.y),
            Side::Right => Point::new(p.y, p.x),
            Side::Left => Point::new(p.y, -p.x),
        }
    }

    fn world(self, c: Point) -> Point {
        match self {
            Side::Below => c,
            Side::Above => Point::new(c.x, -c.y),
            Side::Right => Point::new(c.y, c.x),
            Side::Left => Point::new(-c.y, c.x),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Joined {
    side: Side,
    host: RoundedRect,
    limb: RoundedRect,
}

impl Joined {
    #[must_use]
    pub fn new(host: RoundedRect, limb: RoundedRect) -> Option<Joined> {
        if !host.rect.inflate(TOUCH).intersects(&limb.rect) {
            return None;
        }
        let host = clamped(host.rect, host.radii);
        let limb = clamped(limb.rect, limb.radii);
        Side::ALL
            .into_iter()
            .find_map(|side| Joined::along(side, host, limb))
    }

    fn along(side: Side, host: RoundedRect, limb: RoundedRect) -> Option<Joined> {
        let host = mapped(host, |p| side.canon(p));
        let limb = mapped(limb, |p| side.canon(p));
        let seam = host.rect.max_y().round();
        let touches = (limb.rect.min_y() - seam).abs() <= TOUCH
            && limb.rect.max_y() > seam
            && seam > host.rect.min_y();
        let square = host.radii.bottom_left <= 0.0
            && host.radii.bottom_right <= 0.0
            && limb.radii.top_left <= 0.0
            && limb.radii.top_right <= 0.0;
        let within = limb.rect.min_x() - limb.radii.bottom_left >= host.rect.min_x()
            && limb.rect.max_x() + limb.radii.bottom_right <= host.rect.max_x()
            && limb.rect.width() > 0.0;
        if !(touches && square && within) {
            return None;
        }
        let host_rect = Rect::new(
            host.rect.min_x(),
            host.rect.min_y(),
            host.rect.width(),
            seam - host.rect.min_y(),
        );
        let limb_rect = Rect::new(
            limb.rect.min_x(),
            seam,
            limb.rect.width(),
            limb.rect.max_y() - seam,
        );
        Some(Joined {
            side,
            host: clamped(host_rect, host.radii),
            limb: clamped(limb_rect, limb.radii),
        })
    }

    #[must_use]
    pub fn host(&self) -> RoundedRect {
        mapped(self.host, |c| self.side.world(c))
    }

    #[must_use]
    pub fn limb(&self) -> RoundedRect {
        mapped(self.limb, |c| self.side.world(c))
    }

    #[must_use]
    pub fn fillets(&self) -> [Option<(Rect, RoundedRect)>; 2] {
        let limb = self.limb;
        [
            (limb.rect.min_x(), limb.radii.bottom_left, -1.0),
            (limb.rect.max_x(), limb.radii.bottom_right, 1.0),
        ]
        .map(|(edge, radius, outward)| (radius > 0.0).then(|| self.fillet(edge, radius, outward)))
    }

    fn fillet(&self, edge: f32, radius: f32, outward: f32) -> (Rect, RoundedRect) {
        let seam = self.host.rect.max_y();
        let near = edge + outward * radius;
        let square = Rect::new(edge.min(near), seam, radius, radius);
        let hollow = RoundedRect::circle(Point::new(near, seam + radius), radius);
        (
            mapped(RoundedRect::rect(square), |c| self.side.world(c)).rect,
            mapped(hollow, |c| self.side.world(c)),
        )
    }

    pub(crate) fn translate(self, by: Point) -> Joined {
        let by = self.side.canon(by);
        Joined {
            host: RoundedRect::new(self.host.rect.translate(by), self.host.radii),
            limb: RoundedRect::new(self.limb.rect.translate(by), self.limb.radii),
            ..self
        }
    }

    pub(crate) fn canonical_host(&self) -> RoundedRect {
        self.host
    }

    pub(crate) fn canonical_limb(&self) -> [f32; 6] {
        let limb = self.limb;
        [
            self.side.code(),
            limb.rect.min_x(),
            limb.rect.max_x(),
            limb.rect.height(),
            limb.radii.bottom_left,
            limb.radii.bottom_right,
        ]
    }
}

fn clamped(rect: Rect, radii: CornerRadii) -> RoundedRect {
    RoundedRect::new(rect, radii.clamp_for_size(rect.size))
}

fn mapped(shape: RoundedRect, map: impl Fn(Point) -> Point) -> RoundedRect {
    let corners = shape.rect.corners().map(&map);
    let least = corners.into_iter().fold(corners[0], Point::min);
    let most = corners.into_iter().fold(corners[0], Point::max);
    let rect = Rect::new(least.x, least.y, most.x - least.x, most.y - least.y);
    let centre = rect.center();
    let mut radii = [0.0; 4];
    for (corner, radius) in corners.into_iter().zip(shape.radii.to_array()) {
        let slot = match (corner.x > centre.x, corner.y > centre.y) {
            (false, false) => 0,
            (true, false) => 1,
            (true, true) => 2,
            (false, true) => 3,
        };
        radii[slot] = radius;
    }
    RoundedRect::new(
        rect,
        CornerRadii::new(radii[0], radii[1], radii[2], radii[3]),
    )
}
