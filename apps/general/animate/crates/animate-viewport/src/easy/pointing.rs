use animate_stage::Lens;
use rikka_app::prelude::*;
use rikka_model::glam::Vec3;

const STEEP: f32 = 0.82;
const GRAZING: f32 = 1e-4;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Way {
    Plane { origin: Vec3, normal: Vec3 },
    Push { ahead: Vec3 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Follow {
    target: Vec3,
    offset: Point,
    way: Way,
    last: Point,
}

fn level(lens: &Lens, point: Vec3) -> Vec3 {
    let ahead = lens.ahead(point);
    match ahead.y.abs() > STEEP {
        true => ahead,
        false => Vec3::new(ahead.x, 0.0, ahead.z).normalize_or(ahead),
    }
}

fn on_plane(origin: Vec3, normal: Vec3, (from, heading): (Vec3, Vec3)) -> Option<Vec3> {
    let facing = heading.dot(normal);
    if facing.abs() < GRAZING {
        return None;
    }
    let along = (origin - from).dot(normal) / facing;
    (along > 0.0).then(|| from + heading * along)
}

impl Follow {
    pub(crate) fn new(lens: &Lens, point: Vec3, marker: Point, press: Point) -> Self {
        let way = Way::Plane {
            origin: point,
            normal: level(lens, point),
        };
        Self::with_way(way, point, marker, press)
    }

    pub(crate) fn facing(lens: &Lens, point: Vec3, marker: Point, press: Point) -> Self {
        let way = Way::Plane {
            origin: point,
            normal: lens.ahead(point),
        };
        Self::with_way(way, point, marker, press)
    }

    pub(crate) fn pushed(lens: &Lens, point: Vec3, marker: Point, press: Point) -> Self {
        let way = Way::Push {
            ahead: level(lens, point),
        };
        Self::with_way(way, point, marker, press)
    }

    fn with_way(way: Way, point: Vec3, marker: Point, press: Point) -> Self {
        Self {
            target: point,
            offset: marker - press,
            way,
            last: press,
        }
    }

    pub(crate) fn target(&self) -> Vec3 {
        self.target
    }

    pub(crate) fn advance(&mut self, lens: &Lens, at: Point) -> Vec3 {
        let pointed = at + self.offset;
        self.target = match self.way {
            Way::Plane { origin, normal } => {
                on_plane(origin, normal, lens.ray(pointed)).unwrap_or(self.target)
            }
            Way::Push { ahead } => {
                let (_, _, pixel) = lens.sideways(self.target);
                self.target + ahead * ((self.last.y - at.y) * pixel)
            }
        };
        self.last = at;
        self.target
    }
}
