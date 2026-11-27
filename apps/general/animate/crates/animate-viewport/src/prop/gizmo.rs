use animate_cast::PropRole;
use animate_prop::{PropLevels, PropSetting};
use animate_stage::{PropFrame, Stage};
use rikka_app::prelude::*;
use rikka_model::glam::{EulerRot, Quat, Vec3};

use crate::Gesture;
use crate::easy::Follow;
use crate::prop::rig::{Part, Rig};

#[derive(Clone, Copy, Debug)]
enum Motion {
    Slide {
        follow: Follow,
        axis: Option<Vec3>,
    },
    Turn {
        centre: Point,
        last: f32,
        turned: f32,
    },
}

#[derive(Clone, Copy, Debug)]
struct Drag {
    role: PropRole,
    part: Part,
    rig: Rig,
    start: PropLevels,
    motion: Motion,
    moved: Option<PropLevels>,
}

#[derive(Debug, Default)]
pub(crate) struct PropGizmo {
    drag: Option<Drag>,
    lit: Option<Part>,
}

impl PropGizmo {
    pub(crate) fn lit(&self) -> Option<Part> {
        self.drag.as_ref().map(|drag| drag.part).or(self.lit)
    }

    pub(crate) fn press(&mut self, frame: &PropFrame, rect: Rect, at: Point) -> bool {
        let rig = Rig::new(frame, rect);
        let (Some(part), Some(centre)) = (rig.hit(at), rig.lens.project(rig.origin)) else {
            return false;
        };
        let ahead = rig.lens.ahead(rig.origin);
        let motion = match part {
            Part::Free => Motion::Slide {
                follow: Follow::facing(&rig.lens, rig.origin, centre, at),
                axis: None,
            },
            Part::Plane(normal) => Motion::Slide {
                follow: Follow::across(rig.origin, rig.axes[normal], centre, at),
                axis: None,
            },
            Part::Axis(axis) => {
                let along = rig.axes[axis];
                let facing = (ahead - along * ahead.dot(along)).normalize_or(ahead);
                Motion::Slide {
                    follow: Follow::across(rig.origin, facing, centre, at),
                    axis: Some(along),
                }
            }
            Part::Ring(_) => Motion::Turn {
                centre,
                last: angle_of(centre, at),
                turned: 0.0,
            },
        };
        self.drag = Some(Drag {
            role: frame.role,
            part,
            rig,
            start: frame.levels,
            motion,
            moved: None,
        });
        true
    }

    pub(crate) fn travel(&mut self, stage: &mut Stage, frame_at: u32, at: Point) {
        let Some(drag) = &mut self.drag else {
            return;
        };
        let rig = drag.rig;
        let mut levels = drag.start;
        match &mut drag.motion {
            Motion::Slide { follow, axis } => {
                let reached = follow.advance(&rig.lens, at) - rig.origin;
                let shift = match axis {
                    Some(axis) => *axis * reached.dot(*axis),
                    None => reached,
                };
                let position = drag.start.position() + rig.local(shift);
                levels.x = position.x;
                levels.y = position.y;
                levels.z = position.z;
            }
            Motion::Turn {
                centre,
                last,
                turned,
            } => {
                let Part::Ring(axis) = drag.part else {
                    return;
                };
                let now = angle_of(*centre, at);
                let step = wrapped(now - *last);
                *last = now;
                let toward_eye = rig.axes[axis].dot(rig.lens.ahead(rig.origin)) < 0.0;
                *turned += match toward_eye {
                    true => -step,
                    false => step,
                };
                let unit = [Vec3::X, Vec3::Y, Vec3::Z][axis];
                let rotation = Quat::from_axis_angle(unit, *turned) * drag.start.rotation();
                let (yaw, pitch, roll) = rotation.to_euler(EulerRot::YXZ);
                levels.yaw = nearest(yaw.to_degrees(), drag.start.yaw);
                levels.pitch = nearest(pitch.to_degrees(), drag.start.pitch);
                levels.roll = nearest(roll.to_degrees(), drag.start.roll);
            }
        }
        drag.moved = Some(levels);
        let drafts = &mut stage.drafts_mut().props;
        for setting in settings_of(drag.part) {
            drafts.set(drag.role, frame_at, setting, levels.get(setting));
        }
    }

    pub(crate) fn release(&mut self) -> Option<Gesture> {
        let drag = self.drag.take()?;
        let moved = drag.moved.filter(|moved| *moved != drag.start)?;
        Some(Gesture::Arrange {
            role: drag.role,
            settings: settings_of(drag.part)
                .into_iter()
                .map(|setting| (setting, moved.get(setting)))
                .collect(),
        })
    }

    pub(crate) fn cancel(&mut self, stage: &mut Stage) -> bool {
        let Some(drag) = self.drag.take() else {
            return false;
        };
        let drafts = &mut stage.drafts_mut().props;
        for setting in settings_of(drag.part) {
            drafts.clear(drag.role, setting);
        }
        true
    }

    pub(crate) fn hover(
        &mut self,
        frame: Option<&PropFrame>,
        rect: Rect,
        at: Option<Point>,
    ) -> bool {
        let lit = frame
            .zip(at)
            .and_then(|(frame, at)| Rig::new(frame, rect).hit(at));
        std::mem::replace(&mut self.lit, lit) != lit
    }
}

fn settings_of(part: Part) -> [PropSetting; 3] {
    match part {
        Part::Ring(_) => PropSetting::ROTATION,
        Part::Free | Part::Axis(_) | Part::Plane(_) => PropSetting::POSITION,
    }
}

fn angle_of(centre: Point, at: Point) -> f32 {
    (at.y - centre.y).atan2(at.x - centre.x)
}

fn wrapped(radians: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    (radians + PI).rem_euclid(TAU) - PI
}

fn nearest(degrees: f32, near: f32) -> f32 {
    degrees + ((near - degrees) / 360.0).round() * 360.0
}
