use cut_project::{ClipId, PlacementField, Projection, Viewpoint, Visual};
use cut_render::Overrides;
use rikka_app::prelude::*;
use rikka_figure::GRIP_REACH;
use rikka_figure_editing::paint_line;

use super::Stage;
use super::items::Item;
use super::spinning::{Spin, Spinning, Turn};
use super::viewing::Viewing;

const RADIUS: f32 = 64.0;
const HOLLOW: f32 = 0.4;
const SEGMENTS: usize = 72;
const KNOB_DROP: f32 = 28.0;
const KNOB: f32 = 9.0;
const LINE: f32 = 1.0;
const FOLLOWS: f32 = 0.35;
const TIE: f32 = 0.5;
const MIN_SPEED: f32 = 0.3;
const FIELDS: [PlacementField; 3] = [
    PlacementField::TiltX,
    PlacementField::TiltY,
    PlacementField::Rotation,
];

#[derive(Clone, Debug)]
struct Ring {
    field: PlacementField,
    projection: Projection,
    points: Vec<Point>,
    depths: Vec<f64>,
    openness: f32,
}

#[derive(Clone, Debug)]
pub(crate) struct Gimbal {
    clip: ClipId,
    visual: Visual,
    rings: Vec<Ring>,
    pivot: Point,
    knob: Point,
    per_px: f32,
}

#[derive(Clone, Copy, Debug)]
struct Touch {
    ring: usize,
    segment: usize,
    distance: f32,
    openness: f32,
    depth: f64,
}

impl Stage {
    pub(super) fn gimbal(
        &self,
        item: &Item,
        view: Viewpoint,
        overrides: &Overrides,
        viewing: &Viewing,
    ) -> Option<Gimbal> {
        let placement = item.visual.placement();
        if !placement.three_d {
            return None;
        }
        let project = self.shown_project();
        let (_, clip) = project.locate(item.clip)?;
        let frame = self.transport.frame();
        let plane = |field: PlacementField| {
            let ring = placement.ring(field)?;
            project
                .shot_as(clip, frame, item.visual.placed(ring), view, |id| {
                    overrides.appearance(id)
                })
                .map(|shot| shot.projection)
        };
        let planes: Vec<(PlacementField, Projection)> = FIELDS
            .into_iter()
            .filter_map(|field| Some((field, plane(field)?)))
            .collect();
        let (_, face) = planes
            .iter()
            .find(|(field, _)| *field == PlacementField::Rotation)?;
        let pivot = viewing.to_screen(face.map(Point::ZERO)?);
        let stretch = (face.stretch(Point::ZERO)? * viewing.scale).max(f32::EPSILON);
        let radius = RADIUS / stretch;
        let rings = planes
            .into_iter()
            .filter_map(|(field, projection)| Ring::new(field, projection, radius, viewing))
            .collect();
        Some(Gimbal {
            clip: item.clip,
            visual: item.visual.clone(),
            rings,
            pivot,
            knob: pivot + Point::new(0.0, RADIUS + KNOB_DROP),
            per_px: 1.0 / stretch,
        })
    }
}

impl Ring {
    fn new(
        field: PlacementField,
        projection: Projection,
        radius: f32,
        viewing: &Viewing,
    ) -> Option<Ring> {
        let through = projection.homography();
        let locals: Vec<Point> = (0..=SEGMENTS)
            .map(|step| around(step as f32 * 360.0 / SEGMENTS as f32, radius))
            .collect();
        let points = locals
            .iter()
            .map(|local| projection.map(*local).map(|at| viewing.to_screen(at)))
            .collect::<Option<Vec<Point>>>()?;
        let area: f32 = points
            .windows(2)
            .map(|pair| pair[0].x * pair[1].y - pair[1].x * pair[0].y)
            .sum();
        Some(Ring {
            field,
            depths: locals.iter().map(|local| through.depth(*local)).collect(),
            openness: (area.abs() / 2.0 / (std::f32::consts::PI * RADIUS * RADIUS)).min(1.0),
            projection,
            points,
        })
    }

    fn touches(&self, ring: usize, position: Point) -> impl Iterator<Item = Touch> + '_ {
        self.points
            .windows(2)
            .enumerate()
            .map(move |(segment, pair)| Touch {
                ring,
                segment,
                distance: distance_to(pair[0], pair[1], position),
                openness: self.openness,
                depth: (self.depths[segment] + self.depths[segment + 1]) / 2.0,
            })
            .filter(|touch| touch.distance <= GRIP_REACH)
    }
}

impl Gimbal {
    pub(crate) fn grab(&self, at: Point, position: Point) -> Option<Spinning> {
        let placement = self.visual.placement();
        let spin = match self.knob.distance(position) <= GRIP_REACH {
            true => Spin::Deepening {
                from: position.y,
                depth: placement.depth,
                per_px: self.per_px,
            },
            false => {
                if self.pivot.distance(position) < RADIUS * HOLLOW {
                    return None;
                }
                let touch = self
                    .rings
                    .iter()
                    .enumerate()
                    .flat_map(|(index, ring)| ring.touches(index, position))
                    .min_by(nearer)?;
                let ring = &self.rings[touch.ring];
                let turn = match (ring.openness >= FOLLOWS, ring.projection.unmap(at)) {
                    (true, Some(local)) => Turn::Following {
                        projection: ring.projection.clone(),
                        last: angle(local),
                    },
                    _ => {
                        let (from, to) =
                            (ring.points[touch.segment], ring.points[touch.segment + 1]);
                        let along = to - from;
                        let length = along.x.hypot(along.y).max(f32::EPSILON);
                        let step = std::f32::consts::TAU / SEGMENTS as f32;
                        Turn::Sliding {
                            from: position,
                            along: along * (1.0 / length),
                            speed: (length / step).max(RADIUS * MIN_SPEED),
                        }
                    }
                };
                Spin::Turning {
                    field: ring.field,
                    base: placement.value(ring.field).scalar()?,
                    total: 0.0,
                    turn,
                }
            }
        };
        Some(Spinning::new(
            self.clip,
            self.visual.clone(),
            spin,
            position,
        ))
    }

    pub(crate) fn paint(&self, ctx: &mut PaintCtx<'_>) {
        let palette = palette();
        for ring in &self.rings {
            let color = match ring.field {
                PlacementField::TiltX => palette.danger,
                PlacementField::TiltY => palette.success,
                _ => palette.accent,
            };
            for pair in ring.points.windows(2) {
                paint_line(ctx, pair[0], pair[1], color);
            }
        }
        paint_line(ctx, self.pivot, self.knob, palette.text_muted);
        ctx.draw_quad(
            Quad::new(Rect::from_center(self.knob, Size::new(KNOB, KNOB)))
                .fill(palette.surface)
                .stroke(LINE, palette.text),
        );
    }
}

fn nearer(one: &Touch, other: &Touch) -> std::cmp::Ordering {
    match (one.distance - other.distance).abs() <= TIE {
        true => other
            .openness
            .total_cmp(&one.openness)
            .then(one.depth.total_cmp(&other.depth)),
        false => one.distance.total_cmp(&other.distance),
    }
}

fn around(degrees: f32, radius: f32) -> Point {
    let (sin, cos) = degrees.to_radians().sin_cos();
    Point::new(radius * cos, radius * sin)
}

pub(crate) fn angle(local: Point) -> f32 {
    local.y.atan2(local.x).to_degrees()
}

fn distance_to(from: Point, to: Point, position: Point) -> f32 {
    let along = to - from;
    let length = along.x * along.x + along.y * along.y;
    let t = match length > f32::EPSILON {
        true => {
            let offset = position - from;
            ((offset.x * along.x + offset.y * along.y) / length).clamp(0.0, 1.0)
        }
        false => 0.0,
    };
    (from + along * t).distance(position)
}
