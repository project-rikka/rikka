use cut_project::{Camera, CameraField, Clip, Placement, Property};
use rikka_app::prelude::*;
use rikka_effects::{Scale, Unit};

use super::keys::Keying;
use super::rows::{group_with, number_with, pair_with, sliding_with};
use crate::wording;

const FARTHEST: f32 = 100_000.0;
const MAX_ZOOM: f32 = 400.0;

pub(crate) fn camera(clip: &Clip, camera: Camera, keying: Keying) -> impl View {
    let diamond = |field: CameraField| Some(keying.on(clip, Property::Camera(field)));
    let pixels = || {
        Scale::new(-FARTHEST..=FARTHEST)
            .unit(Unit::Pixels)
            .decimals(1)
    };
    let degrees = || Scale::new(-180.0..=180.0).unit(Unit::Degrees).decimals(1);
    let rows = vec![
        pair_with(
            wording::camera_field(CameraField::Interest),
            camera.interest,
            pixels(),
            (
                |camera: &mut Camera, x: f32| camera.interest.x = x,
                |camera: &mut Camera, y: f32| camera.interest.y = y,
            ),
            diamond(CameraField::Interest),
        ),
        number_with(
            wording::camera_field(CameraField::Dolly),
            camera.dolly,
            pixels(),
            |camera: &mut Camera, dolly: f32| camera.dolly = dolly,
            diamond(CameraField::Dolly),
        ),
        sliding_with(
            wording::camera_field(CameraField::Zoom),
            camera.zoom * 100.0,
            Scale::new(Placement::MIN_ZOOM * 100.0..=MAX_ZOOM)
                .unit(Unit::Percent)
                .decimals(1),
            |camera: &mut Camera, zoom: f32| camera.zoom = zoom / 100.0,
            diamond(CameraField::Zoom),
        ),
        sliding_with(
            wording::camera_field(CameraField::Roll),
            camera.roll,
            degrees(),
            |camera: &mut Camera, roll: f32| camera.roll = roll,
            diamond(CameraField::Roll),
        ),
        sliding_with(
            wording::camera_field(CameraField::Pan),
            camera.pan,
            degrees(),
            |camera: &mut Camera, pan: f32| camera.pan = pan,
            diamond(CameraField::Pan),
        ),
        sliding_with(
            wording::camera_field(CameraField::Tilt),
            camera.tilt,
            degrees(),
            |camera: &mut Camera, tilt: f32| camera.tilt = tilt,
            diamond(CameraField::Tilt),
        ),
        sliding_with(
            wording::camera_field(CameraField::Angle),
            camera.angle,
            Scale::new(Camera::ANGLES).unit(Unit::Degrees).decimals(1),
            |camera: &mut Camera, angle: f32| camera.angle = angle,
            diamond(CameraField::Angle),
        ),
    ];
    group_with(tr!("cut.camera.group"), Some(keying.group(clip)), rows)
}
