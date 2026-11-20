use rikka_model::Bounds;
use rikka_model::glam::{EulerRot, Mat4, Quat, Vec3};
use rikka_model::mmd::{MMD_UNIT_SCALE, VmdCameraFrame, VmdCameraInterpolation, VmdCameraPose};
use rikka_renderer_3d::{Camera, orthographic_reversed, perspective_reversed_infinite};

use crate::mmd_axis::MmdAxis;

const RESTING_HEIGHT: f32 = 10.0 * MMD_UNIT_SCALE;
const RESTING_DISTANCE: f32 = 45.0 * MMD_UNIT_SCALE;
const RESTING_FOV_DEGREES: f32 = 30.0;
const NEAREST: f32 = 0.02;
const FARTHEST: f32 = 500.0;
const CLOSEST: f32 = 0.05;
const TURN_PER_PIXEL: f32 = 0.006;
const DOLLY_PER_NOTCH: f32 = 1.12;
const MARGIN: f32 = 1.15;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shot {
    target: Vec3,
    yaw: f32,
    pitch: f32,
    roll: f32,
    distance: f32,
    fov: f32,
    perspective: bool,
}

impl Default for Shot {
    fn default() -> Self {
        Self {
            target: Vec3::new(0.0, RESTING_HEIGHT, 0.0),
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
            distance: RESTING_DISTANCE,
            fov: RESTING_FOV_DEGREES.to_radians(),
            perspective: true,
        }
    }
}

impl From<VmdCameraPose> for Shot {
    fn from(pose: VmdCameraPose) -> Self {
        Self {
            target: pose.target,
            yaw: -pose.angles.y,
            pitch: -pose.angles.x,
            roll: -pose.angles.z,
            distance: -pose.distance,
            fov: pose.fov,
            perspective: pose.perspective,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Orbit {
    pub yaw: f32,
    pub elevation: f32,
    pub roll: f32,
    pub distance: f32,
    pub fov: f32,
}

impl Shot {
    pub fn placed(target: Vec3, orbit: Orbit) -> Self {
        Self {
            target,
            yaw: orbit.yaw.to_radians(),
            pitch: -orbit.elevation.to_radians(),
            roll: -orbit.roll.to_radians(),
            distance: orbit.distance,
            fov: orbit.fov.to_radians(),
            perspective: true,
        }
    }

    pub fn target(&self) -> Vec3 {
        self.target
    }

    pub fn orbit(&self) -> Orbit {
        Orbit {
            yaw: self.yaw.to_degrees(),
            elevation: -self.pitch.to_degrees(),
            roll: -self.roll.to_degrees(),
            distance: self.distance,
            fov: self.fov.to_degrees(),
        }
    }

    pub fn framing(bounds: Bounds) -> Self {
        let resting = Self::default();
        let radius = bounds.radius().max(CLOSEST);
        Self {
            target: bounds.center(),
            distance: radius / (resting.fov * 0.5).sin() * MARGIN,
            ..resting
        }
    }

    fn rotation(&self) -> Quat {
        Quat::from_euler(EulerRot::YXZ, self.yaw, self.pitch, self.roll)
    }

    pub fn eye(&self) -> Vec3 {
        self.target + self.rotation() * Vec3::new(0.0, 0.0, self.distance)
    }

    pub fn camera(&self, aspect: f32) -> Camera {
        let rotation = self.rotation();
        let placed = Mat4::from_rotation_translation(rotation, self.eye());
        let camera = Camera::default().view(placed.inverse());
        if self.perspective {
            return camera.projection(perspective_reversed_infinite(self.fov, aspect, NEAREST));
        }
        let half_height = self.distance.abs().max(CLOSEST) * (self.fov * 0.5).tan();
        let half_width = half_height * aspect;
        camera.projection(orthographic_reversed(
            -half_width,
            half_width,
            -half_height,
            half_height,
            NEAREST,
            FARTHEST,
        ))
    }

    pub fn turn(&mut self, x: f32, y: f32) {
        self.yaw -= x * TURN_PER_PIXEL;
        self.pitch -= y * TURN_PER_PIXEL;
    }

    pub fn pan(&mut self, x: f32, y: f32, height: f32) {
        let across =
            2.0 * self.distance.abs().max(CLOSEST) * (self.fov * 0.5).tan() / height.max(1.0);
        let rotation = self.rotation();
        self.target += (rotation * Vec3::X * -x + rotation * Vec3::Y * y) * across;
    }

    pub fn dolly(&mut self, notches: f32) {
        let reach = (self.distance.abs() * DOLLY_PER_NOTCH.powf(-notches)).max(CLOSEST);
        self.distance = reach.copysign(self.distance);
    }
}

impl Shot {
    pub fn keyed(&self, frame: u32, interpolation: VmdCameraInterpolation) -> VmdCameraFrame {
        VmdCameraFrame {
            frame,
            distance: -self.distance,
            target: self.target,
            rotation: -Vec3::new(self.pitch, self.yaw, self.roll),
            interpolation,
            angle: self.fov.to_degrees().round().clamp(1.0, 179.0) as u32,
            perspective: self.perspective,
        }
    }

    pub fn turn_about(&mut self, axis: MmdAxis, radians: f32) {
        match axis {
            MmdAxis::X => self.pitch += radians,
            MmdAxis::Y => self.yaw += radians,
            MmdAxis::Z => self.roll -= radians,
        }
    }

    pub fn slide_along(&mut self, axis: MmdAxis, units: f32) {
        self.target += self.rotation() * axis.slide(units);
    }
}
