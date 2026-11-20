use glam::camera::rh::view::look_at_mat4;
use glam::{Mat4, Vec3, Vec4};

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub view: Mat4,
    pub projection: Mat4,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            view: Mat4::IDENTITY,
            projection: Mat4::IDENTITY,
        }
    }
}

impl Camera {
    pub fn look_at(eye: Vec3, target: Vec3, up: Vec3, fov_y: f32, aspect: f32, near: f32) -> Self {
        Self {
            view: look_at_mat4(eye, target, up),
            projection: perspective_reversed_infinite(fov_y, aspect, near),
        }
    }

    pub fn view(mut self, view: Mat4) -> Self {
        self.view = view;
        self
    }

    pub fn projection(mut self, projection: Mat4) -> Self {
        self.projection = projection;
        self
    }

    pub fn view_projection(&self) -> Mat4 {
        self.projection * self.view
    }

    pub fn position(&self) -> Vec3 {
        self.view.inverse().w_axis.truncate()
    }

    pub fn forward(&self) -> Vec3 {
        -Vec3::new(self.view.x_axis.z, self.view.y_axis.z, self.view.z_axis.z)
            .normalize_or(-Vec3::Z)
    }

    pub fn near(&self) -> f32 {
        let m = self.projection.to_cols_array_2d();
        let (a, b) = (m[2][2], m[3][2]);
        let (c, d) = (m[2][3], m[3][3]);
        let denominator = a - c;
        let epsilon = 1e-6 * a.abs().max(c.abs()).max(1.0);
        match denominator.abs() < epsilon {
            true => 1e-4,
            false => ((d - b) / denominator).abs().max(1e-4),
        }
    }

    pub fn frustum_planes(&self) -> [Vec4; 6] {
        let m = self.view_projection().transpose();
        let (x, y, z, w) = (m.x_axis, m.y_axis, m.z_axis, m.w_axis);
        [w + x, w - x, w + y, w - y, w - z, z].map(normalize_plane)
    }
}

pub(crate) fn sphere_in_frustum(planes: &[Vec4; 6], center: Vec3, radius: f32) -> bool {
    !planes
        .iter()
        .any(|plane| plane.truncate().dot(center) + plane.w < -radius)
}

fn normalize_plane(plane: Vec4) -> Vec4 {
    let length = plane.truncate().length();
    match length > 1e-6 {
        true => plane / length,
        false => Vec4::new(0.0, 0.0, 0.0, f32::INFINITY),
    }
}

pub fn perspective_reversed_infinite(fov_y: f32, aspect: f32, near: f32) -> Mat4 {
    let focal = 1.0 / (fov_y * 0.5).tan();
    Mat4::from_cols(
        Vec4::new(focal / aspect.max(1e-6), 0.0, 0.0, 0.0),
        Vec4::new(0.0, -focal, 0.0, 0.0),
        Vec4::new(0.0, 0.0, 0.0, -1.0),
        Vec4::new(0.0, 0.0, near, 0.0),
    )
}

pub fn orthographic_reversed(
    left: f32,
    right: f32,
    bottom: f32,
    top: f32,
    near: f32,
    far: f32,
) -> Mat4 {
    let width = (right - left).max(1e-6);
    let height = (top - bottom).max(1e-6);
    let depth = (far - near).max(1e-6);
    Mat4::from_cols(
        Vec4::new(2.0 / width, 0.0, 0.0, 0.0),
        Vec4::new(0.0, -2.0 / height, 0.0, 0.0),
        Vec4::new(0.0, 0.0, 1.0 / depth, 0.0),
        Vec4::new(
            -(right + left) / width,
            (top + bottom) / height,
            far / depth,
            1.0,
        ),
    )
}
