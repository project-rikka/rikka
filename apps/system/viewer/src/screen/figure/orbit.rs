use rikka_model::Bounds;
use rikka_model::glam::Vec3;
use rikka_renderer_3d::Camera;

const FIELD_OF_VIEW: f32 = 0.8;
const TILT: f32 = 0.18;
const MARGIN: f32 = 1.25;
const NEAREST: f32 = 0.05;
const FARTHEST: f32 = 40.0;
const PITCH_LIMIT: f32 = 1.5;
const TURN_PER_PIXEL: f32 = 0.008;
const DOLLY_PER_NOTCH: f32 = 1.15;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Orbit {
    target: Vec3,
    yaw: f32,
    pitch: f32,
    distance: f32,
    radius: f32,
}

impl Default for Orbit {
    fn default() -> Self {
        Self::framing(Bounds {
            min: Vec3::splat(-0.5),
            max: Vec3::splat(0.5),
        })
    }
}

impl Orbit {
    pub(crate) fn framing(bounds: Bounds) -> Self {
        let radius = bounds.radius().max(f32::EPSILON);
        Self {
            target: bounds.center(),
            yaw: 0.0,
            pitch: TILT,
            distance: radius / (FIELD_OF_VIEW * 0.5).sin() * MARGIN,
            radius,
        }
    }

    pub(crate) fn turn(&mut self, x: f32, y: f32) {
        self.yaw -= x * TURN_PER_PIXEL;
        self.pitch = (self.pitch + y * TURN_PER_PIXEL).clamp(-PITCH_LIMIT, PITCH_LIMIT);
    }

    pub(crate) fn pan(&mut self, x: f32, y: f32, height: f32) {
        let across = 2.0 * self.distance * (FIELD_OF_VIEW * 0.5).tan() / height.max(1.0);
        let (right, up) = self.axes();
        self.target += (right * -x + up * y) * across;
    }

    pub(crate) fn dolly(&mut self, notches: f32) {
        self.distance = (self.distance * DOLLY_PER_NOTCH.powf(-notches))
            .clamp(self.radius * NEAREST, self.radius * FARTHEST);
    }

    pub(crate) fn camera(&self, aspect: f32) -> Camera {
        Camera::look_at(
            self.eye(),
            self.target,
            Vec3::Y,
            FIELD_OF_VIEW,
            aspect,
            (self.radius * 0.01).max(f32::MIN_POSITIVE),
        )
    }

    fn eye(&self) -> Vec3 {
        let (pitch, yaw) = (self.pitch, self.yaw);
        let facing = Vec3::new(
            pitch.cos() * yaw.sin(),
            pitch.sin(),
            pitch.cos() * yaw.cos(),
        );
        self.target + facing * self.distance
    }

    fn axes(&self) -> (Vec3, Vec3) {
        let forward = (self.target - self.eye()).normalize_or_zero();
        let right = forward.cross(Vec3::Y).normalize_or(Vec3::X);
        (right, right.cross(forward).normalize_or(Vec3::Y))
    }
}
