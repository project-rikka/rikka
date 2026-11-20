use cut_project::{ClipId, Placement, PlacementField, Projection, Redraw, Value, Visual};
use rikka_app::prelude::*;
use rikka_ui::DRAG_SLOP;

use super::gimbal::angle;

const TURN_STEP: f32 = 15.0;

#[derive(Clone, Debug)]
pub(crate) enum Turn {
    Following {
        projection: Projection,
        last: f32,
    },
    Sliding {
        from: Point,
        along: Point,
        speed: f32,
    },
}

#[derive(Clone, Debug)]
pub(crate) enum Spin {
    Turning {
        field: PlacementField,
        base: f32,
        total: f32,
        turn: Turn,
    },
    Deepening {
        from: f32,
        depth: f32,
        per_px: f32,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct Spinning {
    clip: ClipId,
    visual: Visual,
    spin: Spin,
    origin: Point,
    moved: bool,
    reached: Visual,
}

impl Spinning {
    pub(crate) fn new(clip: ClipId, visual: Visual, spin: Spin, origin: Point) -> Spinning {
        Spinning {
            clip,
            reached: visual.clone(),
            visual,
            spin,
            origin,
            moved: false,
        }
    }

    pub(crate) fn preview(&self) -> (ClipId, Visual) {
        (self.clip, self.reached.clone())
    }

    pub(crate) fn has_moved(&self) -> bool {
        self.moved
    }

    pub(crate) fn redraw(&self) -> Redraw {
        match self.spin {
            Spin::Turning { .. } => Redraw::Rotate,
            Spin::Deepening { .. } => Redraw::Move,
        }
    }

    pub(crate) fn drag(&mut self, at: Point, position: Point, stepped: bool) -> bool {
        if !self.moved && self.origin.distance(position) < DRAG_SLOP {
            return false;
        }
        self.moved = true;
        let placement = self.visual.placement();
        let reached = match &mut self.spin {
            Spin::Turning {
                field,
                base,
                total,
                turn,
            } => {
                match turn {
                    Turn::Following { projection, last } => {
                        let Some(local) = projection.unmap(at) else {
                            return true;
                        };
                        let now = angle(local);
                        *total += wrapped(now - *last);
                        *last = now;
                    }
                    Turn::Sliding { from, along, speed } => {
                        let by = position - *from;
                        *total = ((by.x * along.x + by.y * along.y) / *speed).to_degrees();
                    }
                }
                let mut value = *base + *total;
                if stepped {
                    value = (value / TURN_STEP).round() * TURN_STEP;
                }
                placement.with(*field, Value::Scalar(value))
            }
            Spin::Deepening {
                from,
                depth,
                per_px,
            } => Placement {
                depth: *depth - (position.y - *from) * *per_px,
                ..placement
            },
        };
        self.reached = self.visual.placed(reached);
        true
    }
}

fn wrapped(degrees: f32) -> f32 {
    (degrees + 180.0).rem_euclid(360.0) - 180.0
}
