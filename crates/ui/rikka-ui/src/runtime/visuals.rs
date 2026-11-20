use std::time::Instant;

use rikka_animation::{Motion, Property};
use rikka_types::{Color, Point, Rect};

use crate::motion::{Clock, Passage, Pose, motion_key};
use crate::runtime::instance::Instance;
use crate::view::Visual;

const FRAME: u64 = motion_key("rikka:frame");
const OPACITY: u64 = motion_key("rikka:opacity");
const SCALE: u64 = motion_key("rikka:scale");
const OFFSET: u64 = motion_key("rikka:offset");
const OFFSET_Y: u64 = motion_key("rikka:offset:y");
const BACKGROUND: u64 = motion_key("rikka:background");

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Look {
    pub(crate) opacity: f32,
    pub(crate) scale: f32,
    pub(crate) about_anchor: bool,
    pub(crate) nudge: Point,
    pub(crate) background: Option<Color>,
    pub(crate) reveal: Option<Rect>,
    pub(crate) moving: bool,
}

impl Look {
    pub(crate) const PLAIN: Look = Look {
        opacity: 1.0,
        scale: 1.0,
        about_anchor: false,
        nudge: Point::ZERO,
        background: None,
        reveal: None,
        moving: false,
    };
}

pub(crate) fn frame(
    instance: &mut Instance,
    target: Rect,
    transaction: Option<Motion>,
    clock: Clock,
) -> (Rect, bool) {
    let key = FRAME;
    let motion = instance
        .visual
        .as_ref()
        .and_then(|visual| visual.animate)
        .or(transaction);
    if motion.is_none() && instance.frame_curve.is_none() {
        return (target, false);
    }
    let flying = instance.motions.holds(key);
    let turned = match flying {
        true => instance.motions.target::<Rect>(key) != Some(target),
        false => instance.placed.is_some_and(|placed| placed != target),
    };
    match (turned, motion) {
        (true, Some(motion)) => instance.frame_curve = Some(motion.curve(Property::Spatial)),
        (_, _) if !flying => return (target, false),
        _ => {}
    }
    let Some(curve) = instance.frame_curve else {
        instance.motions.forget(key);
        return (target, false);
    };
    let start = instance.placed.unwrap_or(target);
    let (shown, moving) = instance.motions.drive(key, start, target, curve, clock);
    if !moving {
        instance.motions.forget(key);
        instance.frame_curve = None;
    }
    (shown, moving)
}

#[inline]
pub(crate) fn look(
    instance: &mut Instance,
    bounds: Rect,
    clock: Clock,
    debut: Instant,
) -> Option<Look> {
    let visual = instance.visual.as_deref().copied()?;
    Some(animated(instance, visual, bounds, clock, debut))
}

fn animated(
    instance: &mut Instance,
    visual: Visual,
    bounds: Rect,
    clock: Clock,
    debut: Instant,
) -> Look {
    let size = bounds.size;
    let mut moving = false;
    let mut settle =
        |key: u64, value: f32, property: Property, motions: &mut crate::motion::Motions| {
            match visual.animate {
                Some(motion) => {
                    let (shown, still) =
                        motions.drive(key, value, value, motion.curve(property), clock);
                    moving |= still;
                    shown
                }
                None => value,
            }
        };
    let opacity = settle(
        OPACITY,
        visual.opacity.unwrap_or(1.0),
        Property::Effects,
        &mut instance.motions,
    );
    let scale = settle(
        SCALE,
        visual.scale.unwrap_or(1.0),
        Property::Spatial,
        &mut instance.motions,
    );
    let offset = visual.offset.unwrap_or(Point::ZERO);
    let nudge = Point::new(
        settle(OFFSET, offset.x, Property::Spatial, &mut instance.motions),
        settle(OFFSET_Y, offset.y, Property::Spatial, &mut instance.motions),
    );
    let background = visual.background.map(|color| match visual.animate {
        Some(motion) => {
            let (shown, still) = instance.motions.drive(
                BACKGROUND,
                color,
                color,
                motion.curve(Property::Effects),
                clock,
            );
            moving |= still;
            shown
        }
        None => color,
    });

    let mut pose = Pose::REST;
    let mut shown = 1.0;
    if let Some(transition) = visual.transition {
        let inward = instance.leaving.is_none();
        if !inward && instance.passage.is_none() {
            instance.passage = Some(Box::new(Passage::departing()));
        }
        if let Some(passage) = instance.passage.as_deref_mut() {
            let step = passage.advance(&transition, inward, size, clock, debut);
            moving |= step.moving;
            shown = step.shown;
            pose = step.pose;
            match instance.leaving.as_mut() {
                Some(leaving) => leaving.done = !step.moving,
                None if !step.moving => instance.passage = None,
                None => {}
            }
        }
    }

    Look {
        opacity: opacity * pose.opacity,
        scale: scale * pose.scale,
        about_anchor: pose.about_anchor,
        nudge: nudge + pose.nudge,
        background,
        reveal: pose
            .wipe
            .map(|direction| crate::motion::revealed(direction, bounds, shown)),
        moving,
    }
}
