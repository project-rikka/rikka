use animate_body::{Body, HumanBone, HumanPose, Side};
use animate_inspector::FaceTab;
use animate_inspector::PoseEditing;
use animate_inspector::{Card, CardFocus};
use animate_motion::{GazeMode, PoseSample};
use animate_solve::EyeDirection;
use animate_solve::Handle;
use animate_viewport::Marker;
use rikka_app::prelude::skip_view;
use rikka_model::glam::Vec3;

use crate::animate::Animate;

impl Animate {
    pub(crate) fn focus(&mut self, marker: Option<Marker>) {
        self.open_card(marker.and_then(card_for));
    }

    fn ahead_of_face(&self) -> Option<Vec3> {
        let body = self.chosen_body()?;
        let worlds = body.fk(&self.shown_pose(body, &self.sampled_now()?));
        let head = worlds.joint(HumanBone::Head);
        let scale = body.scale();
        Some((head.position + head.rotation * Vec3::Z * scale) / scale)
    }
}

fn card_for(marker: Marker) -> Option<Card> {
    match marker {
        Marker::Eyes => Some(Card::Face(FaceTab::Eyes)),
        Marker::Mouth => Some(Card::Face(FaceTab::Mouth)),
        Marker::Handle(Handle::Head) => Some(Card::Face(FaceTab::Expression)),
        Marker::Handle(Handle::Hand(side)) => Some(Card::Hand(side)),
        Marker::Finger(finger) => Some(Card::Hand(finger.side())),
        Marker::Handle(_) => None,
    }
}

impl CardFocus for Animate {
    fn aim_gaze(&mut self, mode: GazeMode) {
        let ahead = self.ahead_of_face();
        self.touch_with(|pose_sample| {
            let gaze = &mut pose_sample.gaze;
            if mode == GazeMode::Point
                && gaze.target == Vec3::ZERO
                && let Some(ahead) = ahead
            {
                gaze.target = ahead;
            }
            gaze.mode = mode;
        });
    }
    fn card(&self) -> Option<Card> {
        self.controls.card
    }
    fn eye_direction(&mut self, eye: Option<Side>, eye_direction: EyeDirection) {
        let limits = self.limits();
        let eyes = match eye {
            Some(eye) => vec![eye],
            None => Side::BOTH.to_vec(),
        };
        self.touch_with(|pose_sample| {
            for eye in eyes {
                eye_direction.apply(&mut pose_sample.pose, eye, limits);
            }
            pose_sample.gaze.mode = GazeMode::Off;
        });
    }
    fn open_card(&mut self, card: Option<Card>) {
        if self.controls.card == card {
            skip_view();
            return;
        }
        self.controls.card = card;
    }
    fn shown_pose(&self, body: &Body, pose_sample: &PoseSample) -> HumanPose {
        let frame = self.transport.frame();
        pose_sample.still(body, || self.eye().at(frame))
    }
}
