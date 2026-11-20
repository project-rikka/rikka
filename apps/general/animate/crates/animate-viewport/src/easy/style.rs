use animate_body::Side;
use animate_solve::Handle;
use animate_ui::{LEFT_HAND_TINT, PIN_TINT, RIGHT_HAND_TINT};
use rikka_app::prelude::Color;

use crate::easy::part::Marker;

pub(super) const LINE: f32 = 1.4;
pub(super) const CONTROL_REACH: f32 = 6.0;
pub(super) const CONTROL_OUTLINE: f32 = 2.0;
pub(super) const CONTROL_HALO: f32 = 1.5;
pub(super) const LIT_RING: f32 = 4.0;
pub(super) const CHOSEN_GAP: f32 = 3.5;
pub(super) const PIN_OUTLINE: f32 = 2.5;
pub(super) const PIN_DOT: f32 = 0.45;
pub(super) const CROSSHAIR: f32 = 18.0;
pub(super) const CROSSHAIR_REACH: f32 = 12.0;
pub(super) const PATH_DOT: f32 = 2.0;
pub(super) const PATH_STEP: f32 = 3.0;
pub(super) const PATH_KEY: f32 = 7.0;
pub(super) const PATH_REACH: u32 = 60;
pub(super) const TURN_PACE: f32 = 100.0;
pub(super) const TWIST_PACE: f32 = 100.0;
pub(super) const CURL_PACE: f32 = 0.01;
pub(super) const SPREAD_PACE: f32 = 0.01;
pub(super) const OPEN_PACE: f32 = 0.01;
pub(super) const PUPIL: f32 = 0.45;
pub(super) const MARKER_REACH: f32 = 7.0;
pub(super) const MARKER: f32 = 5.0;
pub(super) const RING: f32 = 19.0;
pub(super) const RING_REACH: f32 = 5.0;
pub(super) const RING_LINE: f32 = 2.0;
pub(super) const RING_LIT_LINE: f32 = 3.0;
pub(super) const PATCH_DOT: f32 = 2.5;
pub(super) const BAND: f32 = 1.4;
pub(super) const TARGET: f32 = 7.0;
pub(super) const NAME: f32 = 12.0;
pub(super) const NAME_LINE: f32 = 16.0;
pub(super) const NAME_GAP: f32 = 6.0;

pub(super) const LINKS: Color = Color::rgb(0.3, 0.75, 1.0);
pub(super) const MIDDLE: Color = Color::rgb(1.0, 0.78, 0.2);
pub(super) const LEFT: Color = LEFT_HAND_TINT;
pub(super) const RIGHT: Color = RIGHT_HAND_TINT;
pub(super) const PINNED: Color = PIN_TINT;
pub(super) const GAZE: Color = Color::rgb(0.95, 0.4, 0.85);
pub(super) const MOUTH: Color = Color::rgb(1.0, 0.55, 0.45);
pub(super) const PATH: Color = Color::rgb(1.0, 0.85, 0.3);
pub(super) const HELD: Color = Color::rgb(1.0, 0.82, 0.25);
pub(super) const SNAPPED: Color = Color::rgb(0.4, 1.0, 0.6);

pub(super) fn radius(marker: Marker) -> f32 {
    match marker {
        Marker::Finger(_) => 5.0,
        Marker::Eyes => 8.0,
        Marker::Mouth => 7.0,
        Marker::Handle(handle) => match handle {
            Handle::Centre => 11.0,
            Handle::LowerBody | Handle::UpperBody | Handle::Head => 10.0,
            Handle::Hand(_) | Handle::Foot(_) => 9.0,
            Handle::Shoulder(_) | Handle::UpperArm(_) | Handle::Forearm(_) | Handle::Toes(_) => 8.0,
        },
    }
}

pub(super) fn square(marker: Marker) -> bool {
    marker == Marker::Handle(Handle::Centre)
}

pub(super) fn tint(marker: Marker) -> Color {
    match (marker, marker.side()) {
        (Marker::Eyes, _) => GAZE,
        (Marker::Mouth, _) => MOUTH,
        (_, None) => MIDDLE,
        (_, Some(Side::Left)) => LEFT,
        (_, Some(Side::Right)) => RIGHT,
    }
}
