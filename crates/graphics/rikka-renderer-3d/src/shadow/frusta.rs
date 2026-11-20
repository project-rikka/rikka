use glam::camera::rh::view::look_at_mat4;
use glam::{Mat4, Vec2, Vec3};

use crate::camera::{Camera, orthographic_reversed, perspective_reversed_infinite};
use crate::shadow::{Bounds, CASCADES};

const CUBE_MARGIN: f32 = 1.08;

const EXTENT_STEPS: f32 = 8.0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Projected {
    pub view_projection: Mat4,
    pub world_texel: f32,
    pub depth_range: f32,
    pub near: f32,
    pub projection_scale: f32,
}

pub(crate) fn cascade_splits(near: f32, far: f32, lambda: f32) -> [f32; CASCADES as usize] {
    let ratio = far / near.max(1e-4);
    std::array::from_fn(|index| {
        let fraction = (index + 1) as f32 / CASCADES as f32;
        let logarithmic = near * ratio.powf(fraction);
        let uniform = near + (far - near) * fraction;
        lambda * logarithmic + (1.0 - lambda) * uniform
    })
}

struct Frustum {
    position: Vec3,
    forward: Vec3,
    tan_x: f32,
    tan_y: f32,
}

impl Frustum {
    fn from_camera(camera: &Camera) -> Self {
        let projection = camera.projection.to_cols_array_2d();
        let inverse_view = camera.view.inverse();
        Self {
            position: inverse_view.w_axis.truncate(),
            forward: -inverse_view.z_axis.truncate(),
            tan_x: 1.0 / projection[0][0].abs().max(1e-6),
            tan_y: 1.0 / projection[1][1].abs().max(1e-6),
        }
    }

    fn bounding_sphere(&self, near: f32, far: f32) -> (Vec3, f32) {
        let spread = self.tan_x * self.tan_x + self.tan_y * self.tan_y;
        let along = ((far + near) * (1.0 + spread) * 0.5).min(far);
        let radius = ((far - along).powi(2) + far * far * spread)
            .max((along - near).powi(2) + near * near * spread)
            .sqrt();
        (self.position + self.forward * along, radius.max(1e-3))
    }
}

fn light_up(direction: Vec3) -> Vec3 {
    match direction.y.abs() > 0.99 {
        true => Vec3::Z,
        false => Vec3::Y,
    }
}

pub(crate) fn cascade_view(
    camera: &Camera,
    direction: Vec3,
    near: f32,
    far: f32,
    resolution: u32,
) -> Projected {
    let (centre, radius) = Frustum::from_camera(camera).bounding_sphere(near, far);
    let radius = radius * (1.0 + 2.0 / resolution as f32);

    let direction = direction.normalize_or(Vec3::NEG_Y);
    let up = light_up(direction);

    let texel = radius * 2.0 / resolution as f32;
    let turned = look_at_mat4(Vec3::ZERO, direction, up);
    let local = turned.transform_point3(centre);
    let snapped = Vec3::new(
        (local.x / texel).floor() * texel,
        (local.y / texel).floor() * texel,
        local.z,
    );
    let centre = turned.inverse().transform_point3(snapped);

    let back_off = radius * 2.0;
    let view = look_at_mat4(centre - direction * back_off, centre, up);
    let depth_range = back_off + radius * 2.0;
    let projection = orthographic_reversed(-radius, radius, -radius, radius, 0.0, depth_range);
    Projected {
        view_projection: projection * view,
        world_texel: texel,
        depth_range,
        near: 0.0,
        projection_scale: 0.0,
    }
}

pub(crate) fn light_turn(direction: Vec3) -> Mat4 {
    let direction = direction.normalize_or(Vec3::NEG_Y);
    look_at_mat4(Vec3::ZERO, direction, light_up(direction))
}

fn quantized(extent: f32) -> f32 {
    ((extent.max(1e-3).log2() * EXTENT_STEPS).ceil() / EXTENT_STEPS).exp2()
}

pub(crate) fn figure_view(fitted: Bounds, direction: Vec3, resolution: u32) -> Projected {
    let direction = direction.normalize_or(Vec3::NEG_Y);
    let up = light_up(direction);
    let turned = light_turn(direction);
    let Bounds { low, high } = fitted;
    let margin = 1.0 + 4.0 / resolution as f32;
    let half_x = quantized((high.x - low.x) * 0.5 * margin);
    let half_y = quantized((high.y - low.y) * 0.5 * margin);
    let texel = Vec2::new(half_x, half_y) * 2.0 / resolution as f32;
    let middle = Vec3::new(
        ((low.x + high.x) * 0.5 / texel.x).round() * texel.x,
        ((low.y + high.y) * 0.5 / texel.y).round() * texel.y,
        high.z,
    );
    let depth = (high.z - low.z).max(1e-3);
    let finest = texel.min_element();
    let back_off = depth * 0.25 + finest * 4.0;
    let eye = turned
        .inverse()
        .transform_point3(middle + Vec3::Z * back_off);
    let view = look_at_mat4(eye, eye + direction, up);
    let depth_range = back_off + depth * 4.0;
    let projection = orthographic_reversed(-half_x, half_x, -half_y, half_y, 0.0, depth_range);
    Projected {
        view_projection: projection * view,
        world_texel: texel.max_element(),
        depth_range,
        near: 0.0,
        projection_scale: 0.0,
    }
}

pub(crate) fn spot_view(
    position: Vec3,
    direction: Vec3,
    range: f32,
    outer_angle: f32,
    resolution: u32,
) -> Projected {
    let direction = direction.normalize_or(Vec3::NEG_Y);
    let view = look_at_mat4(position, position + direction, light_up(direction));
    let fov = (outer_angle * 2.0).clamp(0.05, 3.0);
    let near = range * 0.01;
    let projection = perspective_reversed_infinite(fov, 1.0, near);
    let scale = 1.0 / (fov * 0.5).tan();
    Projected {
        view_projection: projection * view,
        world_texel: range * 2.0 / (scale * resolution as f32),
        depth_range: range,
        near,
        projection_scale: scale,
    }
}

pub(crate) fn cube_views(position: Vec3, range: f32, resolution: u32) -> [Projected; 6] {
    const FACES: [(Vec3, Vec3); 6] = [
        (Vec3::X, Vec3::NEG_Y),
        (Vec3::NEG_X, Vec3::NEG_Y),
        (Vec3::Y, Vec3::Z),
        (Vec3::NEG_Y, Vec3::NEG_Z),
        (Vec3::Z, Vec3::NEG_Y),
        (Vec3::NEG_Z, Vec3::NEG_Y),
    ];
    let near = (range * 0.005).max(1e-3);
    let projection = perspective_reversed_infinite(2.0 * CUBE_MARGIN.atan(), 1.0, near);
    FACES.map(|(forward, up)| Projected {
        view_projection: projection * look_at_mat4(position, position + forward, up),
        world_texel: range * 2.0 / resolution as f32,
        depth_range: range,
        near,
        projection_scale: 1.0 / CUBE_MARGIN,
    })
}

pub(crate) fn biases(world_texel: f32) -> (f32, f32) {
    let depth = (world_texel * 1.5e-3).clamp(1e-5, 5e-3);
    let normal = world_texel * 2.5;
    (depth, normal)
}
