use animate_body::{Expression, HumanPose, Side, Worlds};
use animate_hold::HoldId;
use animate_motion::{GazeMode, Goal, Part, Pinned, PoseSample};
use animate_solve::{
    DragScope, DragSession, EyeDirection, Finger, Handle, Pin, look, rotate, twist,
};
use animate_stage::Posture;
use rikka_app::prelude::*;
use rikka_model::glam::Vec3;
use rikka_ui::DRAG_SLOP;

use crate::Gesture;
use crate::easy::clearance::Clearance;
use crate::easy::grasp::Grasping;
use crate::easy::part::{Marker, part_of};
use crate::easy::pointing::Follow;
use crate::easy::rotation_drag::RotationDrag;
use crate::easy::scene::{Control, Pointed, Scene};
use crate::easy::style::{CROSSHAIR_REACH, CURL_PACE, MARKER_REACH, OPEN_PACE, SPREAD_PACE};
use crate::easy::twist_drag::TwistDrag;

const SPREAD_PROBE: f32 = 0.1;

#[derive(Debug, Default)]
pub(crate) struct EasyDragState {
    grab: Option<Grab>,
    hover: Option<Pointed>,
}

#[derive(Debug)]
struct Grab {
    before: PoseSample,
    after: PoseSample,
    press: Point,
    moved: bool,
    restore: Option<Box<Posture>>,
    kind: Kind,
}

impl Grab {
    fn marker(&self) -> Option<Marker> {
        match &self.kind {
            Kind::Move(moving) => Some(Marker::Handle(moving.handle)),
            Kind::Twist { handle, .. } | Kind::Rotate { handle, .. } => {
                Some(Marker::Handle(*handle))
            }
            Kind::Flex { finger, .. } => Some(Marker::Finger(*finger)),
            Kind::EyeDirection(_) => Some(Marker::Eyes),
            Kind::Open { .. } => Some(Marker::Mouth),
            Kind::Gaze(_) | Kind::Grasp(_) => None,
        }
    }
}

#[derive(Debug)]
enum Kind {
    Move(Box<Moving>),
    Twist {
        handle: Handle,
        twist_drag: TwistDrag,
        pins: Vec<Pin>,
    },
    Rotate {
        handle: Handle,
        rotation: RotationDrag,
        pins: Vec<Pin>,
    },
    Flex {
        finger: Finger,
        spread: f32,
    },
    EyeDirection(Box<EyeDrag>),
    Open {
        from: f32,
    },
    Gaze(Follow),
    Grasp(Box<Grasping>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PressOutcome {
    Ignored,
    Grabbed,
    ReleaseHold(HoldId),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ReleaseOutcome {
    pub marker: Option<Marker>,
    pub gesture: Option<Gesture>,
}

impl ReleaseOutcome {
    pub(crate) fn part(&self) -> Option<Part> {
        self.marker.and_then(part_of)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum CancelOutcome {
    Restore(Box<Posture>),
    Release,
    Express(Expression, f32),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum DragUpdate {
    Pose(Box<Posture>),
    Express(Expression, f32),
}

#[derive(Debug)]
struct Moving {
    handle: Handle,
    session: DragSession,
    follow: Follow,
    origin: Vec3,
    clearance: Option<Clearance>,
    scope: DragScope,
}

#[derive(Debug)]
struct EyeDrag {
    follow: Follow,
    start: Worlds,
}

impl Moving {
    fn lifted(&self, scene: &Scene, target: Vec3) -> Vec3 {
        self.clearance
            .as_ref()
            .map_or(target, |clearance| clearance.lift(scene, target))
    }

    fn travel(&mut self, scene: &Scene, at: Point) -> HumanPose {
        let target = self.follow.advance(&scene.lens, at);
        self.reach(self.lifted(scene, target))
    }

    fn again(&mut self, scene: &Scene) -> HumanPose {
        self.reach(self.lifted(scene, self.follow.target()))
    }

    fn reach(&mut self, target: Vec3) -> HumanPose {
        self.session.update(target - self.origin, self.scope)
    }
}

fn spread_sign(scene: &Scene, finger: Finger) -> f32 {
    let body = &scene.body;
    let lens = &scene.lens;
    let pose = &scene.pose_sample.pose;
    let spread = finger.flex(body, pose, 0.0, SPREAD_PROBE, scene.limits());
    let tip_on_screen = |pose: &HumanPose| lens.project(finger.tip(body, &body.fk(pose)));
    match (tip_on_screen(pose), tip_on_screen(&spread)) {
        (Some(from), Some(to)) if to.x < from.x => -1.0,
        _ => 1.0,
    }
}

fn recruited(shift: bool) -> DragScope {
    match shift {
        true => DragScope::FullBody,
        false => DragScope::Limb,
    }
}

impl EasyDragState {
    pub(super) fn lit(&self) -> Option<Marker> {
        match &self.grab {
            Some(grab) => grab.marker(),
            None => match self.hover {
                Some(Pointed::Marker(marker)) => Some(marker),
                Some(Pointed::Ring(_)) | None => None,
            },
        }
    }

    pub(super) fn lit_ring(&self) -> Option<Side> {
        match &self.grab {
            Some(_) => self.grasping().map(Grasping::side),
            None => match self.hover {
                Some(Pointed::Ring(side)) => Some(side),
                Some(Pointed::Marker(_)) | None => None,
            },
        }
    }

    pub(super) fn holding(&self) -> bool {
        self.grab
            .as_ref()
            .is_some_and(|grab| grab.marker().is_some())
    }

    pub(crate) fn feels(&self) -> bool {
        self.grasping().is_some()
    }

    pub(super) fn grasping(&self) -> Option<&Grasping> {
        match &self.grab.as_ref()?.kind {
            Kind::Grasp(grasping) => Some(grasping),
            _ => None,
        }
    }

    pub(crate) fn drop_grab(&mut self) {
        self.grab = None;
    }

    pub(crate) fn press(
        &mut self,
        scene: &Scene,
        at: Point,
        modifiers: Modifiers,
        restore: Option<Posture>,
    ) -> PressOutcome {
        let sight = scene.projection();
        let lens = &scene.lens;
        let begin = |kind: Kind| Grab {
            before: scene.pose_sample.clone(),
            after: scene.pose_sample.clone(),
            press: at,
            moved: false,
            restore: restore.map(Box::new),
            kind,
        };
        if let Some(hold) = scene
            .grips
            .iter()
            .filter_map(|grip| Some((grip.id, lens.project(grip.point)?)))
            .find(|(_, screen_point)| screen_point.distance(at) <= MARKER_REACH)
            .map(|(hold, _)| hold)
        {
            return PressOutcome::ReleaseHold(hold);
        }
        if let Some((target, spot)) = sight.gaze
            && at.distance(spot) <= CROSSHAIR_REACH
        {
            let follow = Follow::new(lens, target, spot, at);
            self.grab = Some(begin(Kind::Gaze(follow)));
            return PressOutcome::Grabbed;
        }
        let Some(control) = sight.control_at(at) else {
            let Some(grasping) = sight
                .ring_at(at)
                .and_then(|side| Grasping::new(scene, side, at))
            else {
                return PressOutcome::Ignored;
            };
            self.grab = Some(begin(Kind::Grasp(Box::new(grasping))));
            return PressOutcome::Grabbed;
        };
        let kind = match control.marker {
            Marker::Finger(finger) => Kind::Flex {
                finger,
                spread: spread_sign(scene, finger),
            },
            Marker::Eyes => Kind::EyeDirection(Box::new(EyeDrag {
                follow: Follow::facing(lens, control.point, control.at, at),
                start: scene.shown.clone(),
            })),
            Marker::Mouth => Kind::Open {
                from: scene.opened(),
            },
            Marker::Handle(handle) => grasped(scene, handle, &control, at, modifiers),
        };
        self.grab = Some(begin(kind));
        PressOutcome::Grabbed
    }

    pub(crate) fn travel(&mut self, scene: &Scene, at: Point) -> Option<DragUpdate> {
        let grab = self.grab.as_mut()?;
        if !grab.moved && at.distance(grab.press) < DRAG_SLOP {
            return None;
        }
        grab.moved = true;
        let body = &scene.body;
        let limits = scene.limits();
        let displayed = match &mut grab.kind {
            Kind::Move(moving) => {
                grab.after.pose = moving.travel(scene, at);
                grab.after.pose
            }
            Kind::Twist {
                handle,
                twist_drag,
                pins,
            } => {
                let angle = twist_drag.advance(at);
                grab.after.pose = twist(body, &grab.before.pose, *handle, angle, pins, limits);
                grab.after.pose
            }
            Kind::Rotate {
                handle,
                rotation,
                pins,
            } => {
                let delta = rotation.advance(at);
                grab.after.pose = rotate(body, &grab.before.pose, *handle, delta, pins, limits);
                grab.after.pose
            }
            Kind::Flex { finger, spread } => {
                let moved = at - grab.press;
                let (curl, spread) = (moved.y * CURL_PACE, moved.x * *spread * SPREAD_PACE);
                grab.after.pose = finger.flex(body, &grab.before.pose, curl, spread, limits);
                grab.after.pose
            }
            Kind::EyeDirection(glancing) => {
                let target = glancing.follow.advance(&scene.lens, at);
                for eye in Side::BOTH {
                    if let Some(eye_direction) =
                        EyeDirection::aim(body, &glancing.start, eye, target, limits)
                    {
                        eye_direction.apply(&mut grab.after.pose, eye, limits);
                    }
                }
                grab.after.gaze.mode = GazeMode::Off;
                grab.after.pose
            }
            Kind::Open { from } => {
                let opened = (*from + (at.y - grab.press.y) * OPEN_PACE).clamp(0.0, 1.0);
                return Some(DragUpdate::Express(Expression::Aa, opened));
            }
            Kind::Gaze(follow) => {
                let target = follow.advance(&scene.lens, at);
                let gaze = &mut grab.after.gaze;
                gaze.target = target / body.scale();
                let mut pose = grab.after.pose;
                look(body, &mut pose, target, gaze.weight, gaze.head_follow);
                pose
            }
            Kind::Grasp(grasping) => {
                grasping.travel(scene, at);
                return None;
            }
        };
        Some(DragUpdate::Pose(Box::new(Posture {
            pose_sample: grab.after.clone(),
            shown: displayed,
        })))
    }

    fn reach_again(&mut self, scene: &Scene) -> Option<Posture> {
        let grab = self.grab.as_mut().filter(|grab| grab.moved)?;
        let Kind::Move(moving) = &mut grab.kind else {
            return None;
        };
        grab.after.pose = moving.again(scene);
        Some(Posture {
            pose_sample: grab.after.clone(),
            shown: grab.after.pose,
        })
    }

    fn moving(&mut self) -> Option<&mut Moving> {
        match &mut self.grab.as_mut()?.kind {
            Kind::Move(moving) => Some(moving),
            _ => None,
        }
    }

    pub(crate) fn shift(&mut self, scene: &Scene, pressed: bool) -> Option<Posture> {
        let moving = self.moving()?;
        let desired = recruited(pressed);
        if moving.scope == desired {
            return None;
        }
        moving.scope = desired;
        self.reach_again(scene)
    }

    pub(crate) fn widen(&mut self, notches: f32) -> bool {
        let Some(Kind::Grasp(grasping)) = self.grab.as_mut().map(|grab| &mut grab.kind) else {
            return false;
        };
        grasping.widen(notches);
        true
    }

    pub(crate) fn release(&mut self, scene: &Scene) -> ReleaseOutcome {
        let Some(mut grab) = self.grab.take() else {
            return ReleaseOutcome::default();
        };
        if let Kind::Grasp(grasping) = &grab.kind {
            return ReleaseOutcome {
                marker: None,
                gesture: laid(scene, grasping, grab.moved, grab.restore.is_some()),
            };
        }
        let marker = grab.marker();
        if !grab.moved {
            return ReleaseOutcome {
                marker,
                gesture: None,
            };
        }
        if let Some(handle) = marker.and_then(Marker::handle) {
            regoal(scene, handle, &mut grab.after);
        }
        let gesture = match grab.kind {
            Kind::Open { .. } => None,
            _ => Some(Gesture::Pose(Box::new(grab.after))),
        };
        ReleaseOutcome { marker, gesture }
    }

    pub(crate) fn cancel(&mut self) -> Option<CancelOutcome> {
        let grab = self.grab.take().filter(|grab| grab.moved)?;
        match grab.kind {
            Kind::Grasp(_) => None,
            Kind::Open { from } => Some(CancelOutcome::Express(Expression::Aa, from)),
            _ => Some(
                grab.restore
                    .map_or(CancelOutcome::Release, CancelOutcome::Restore),
            ),
        }
    }

    pub(crate) fn hover(&mut self, scene: &Scene, at: Option<Point>) -> bool {
        let hover = at.and_then(|at| scene.projection().pointed_at(at));
        let changed = self.hover != hover;
        self.hover = hover;
        changed
    }
}

fn laid(scene: &Scene, grasping: &Grasping, moved: bool, playback: bool) -> Option<Gesture> {
    if !moved {
        let grip = scene.grip_of(grasping.side())?;
        return Some(Gesture::ReleaseHold(scene.id, grip.id));
    }
    let hold = Box::new(grasping.hold(scene)?);
    Some(match playback {
        true => Gesture::Grasp(hold),
        false => Gesture::Hold(scene.id, hold),
    })
}

fn grasped(
    scene: &Scene,
    handle: Handle,
    control: &Control,
    at: Point,
    modifiers: Modifiers,
) -> Kind {
    let point = control.point;
    let pins = scene.pins();
    let twist_drag = handle
        .twist_axis(&scene.body, &scene.shown)
        .filter(|_| modifiers.control && !modifiers.shift);
    if let Some(axis) = twist_drag {
        return Kind::Twist {
            handle,
            twist_drag: TwistDrag::new(scene, axis, point, at),
            pins,
        };
    }
    if modifiers.control {
        return Kind::Rotate {
            handle,
            rotation: RotationDrag::new(scene, handle, point, at),
            pins,
        };
    }
    let session = DragSession::begin(
        scene.body.clone(),
        scene.pose_sample.pose,
        handle,
        &pins,
        scene.limits(),
    );
    Kind::Move(Box::new(Moving {
        handle,
        session,
        follow: Follow::facing(&scene.lens, point, control.at, at),
        origin: point,
        clearance: Clearance::new(scene, handle, point),
        scope: recruited(modifiers.shift),
    }))
}

fn regoal(scene: &Scene, handle: Handle, after: &mut PoseSample) {
    let Some(limb) = handle.end().filter(|limb| after.pins[limb.index()].pin) else {
        return;
    };
    let body = &scene.body;
    let goal = Goal::from_worlds(&body.fk(&after.pose), limb, body.scale());
    after.pins[limb.index()] = Pinned::new(true, goal);
}
