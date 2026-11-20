use glam::{Mat3, Mat4, Vec2, Vec3};

use crate::camera::Camera;
use crate::composite::DepthOfField;

const LARGEST_BLUR: f32 = 0.035;
const SMALLEST_BLUR: f32 = 0.1;
const CLOSEST_SUBJECT: f32 = 0.25;

const LINEAR_TO_LMS: [[f32; 3]; 3] = [
    [3.90405e-1, 5.49941e-1, 8.92632e-3],
    [7.08416e-2, 9.63172e-1, 1.35775e-3],
    [2.31082e-2, 1.28021e-1, 9.36245e-1],
];
const LMS_TO_LINEAR: [[f32; 3]; 3] = [
    [2.85847e0, -1.62879e0, -2.48910e-2],
    [-2.10182e-1, 1.15820e0, 3.24281e-4],
    [-4.18120e-2, -1.18169e-1, 1.06867e0],
];
const D65_LMS: Vec3 = Vec3::new(0.949237, 1.03542, 1.08728);

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Focus {
    pub scale: f32,
    pub ratio: f32,
    pub limit: f32,
}

impl Focus {
    pub(crate) fn of(lens: &DepthOfField, camera: &Camera, height: u32) -> Option<Self> {
        let usable = lens.aperture.is_finite() && lens.aperture > 0.0 && lens.sensor > 0.0;
        if !usable {
            return None;
        }
        let focal = lens.focal_length(field_of_view(camera.projection)) / 1000.0;
        let focus = lens.focus.max(focal * 1.5);
        let diameter = focal * focal / (lens.aperture * (focus - focal)) * 1000.0;
        let scale = 0.5 * diameter / lens.sensor * height as f32;
        let widest = scale.max(scale * (focus / CLOSEST_SUBJECT - 1.0));
        if !widest.is_finite() || widest < SMALLEST_BLUR {
            return None;
        }
        Some(Self {
            scale,
            ratio: focus / camera.near(),
            limit: LARGEST_BLUR * height as f32,
        })
    }
}

pub(crate) fn field_of_view(projection: Mat4) -> f32 {
    2.0 * (1.0 / projection.y_axis.y.abs().max(1e-6)).atan()
}

pub(crate) fn sun_on_screen(view_projection: Mat4, to_sun: Vec3) -> Option<Vec2> {
    let clip = view_projection * to_sun.extend(0.0);
    if clip.w <= 1e-4 {
        return None;
    }
    Some(clip.truncate().truncate() / clip.w * 0.5 + 0.5)
}

pub(crate) fn sun_bearing(view: Mat4, to_sun: Vec3) -> (Vec2, f32) {
    let seen = view.transform_vector3(to_sun);
    let across = Vec2::new(seen.x, -seen.y).normalize_or(Vec2::Y);
    (across, -seen.z)
}

pub(crate) fn white_balance(temperature: f32, tint: f32) -> Mat3 {
    let warmth = temperature * 1.54;
    let shift = tint * 1.54;
    let x = 0.31271 - warmth * if warmth < 0.0 { 0.1 } else { 0.05 };
    let y = 2.87 * x - 3.0 * x * x - 0.275_095_07 + shift * 0.05;
    let balance = D65_LMS / chromaticity_to_lms(x, y);
    rows(LMS_TO_LINEAR) * Mat3::from_diagonal(balance) * rows(LINEAR_TO_LMS)
}

fn chromaticity_to_lms(x: f32, y: f32) -> Vec3 {
    let tristimulus = Vec3::new(x / y, 1.0, (1.0 - x - y) / y);
    rows([
        [0.7328, 0.4296, -0.1624],
        [-0.7036, 1.6975, 0.0061],
        [0.0030, 0.0136, 0.9834],
    ]) * tristimulus
}

fn rows(values: [[f32; 3]; 3]) -> Mat3 {
    Mat3::from_cols_array_2d(&values).transpose()
}
