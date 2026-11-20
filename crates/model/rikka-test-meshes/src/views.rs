use glam::camera::rh::proj::opengl;
use glam::camera::rh::view::look_at_mat4;
use glam::{Vec2, Vec3};
use rikka_mesh_edit::EditMesh;
use rikka_spatial::{ClipSpace, View, Viewport};

use crate::rng::Rng;

const VIEWPORT: Vec2 = Vec2::new(800.0, 600.0);

#[must_use]
pub fn bounds(mesh: &EditMesh) -> (Vec3, Vec3) {
    let mut low = Vec3::splat(f32::INFINITY);
    let mut high = Vec3::splat(f32::NEG_INFINITY);
    for vert in mesh.verts() {
        let position = mesh.position(vert);
        low = low.min(position);
        high = high.max(position);
    }
    match low.is_finite() && high.is_finite() {
        true => (low, high),
        false => (Vec3::splat(-1.0), Vec3::splat(1.0)),
    }
}

pub fn random_view(mesh: &EditMesh, rng: &mut Rng) -> View {
    let (low, high) = bounds(mesh);
    let center = (low + high) * 0.5;
    let radius = ((high - low).length() * 0.5).max(0.5);
    let direction = rng.direction();
    let up = match direction.cross(Vec3::Z).length_squared() < 1.0e-4 {
        true => Vec3::Y,
        false => Vec3::Z,
    };
    let eye = center + direction * radius * 3.0;
    let camera = look_at_mat4(eye, center, up);
    let projection = match rng.flip() {
        true => opengl::perspective(0.9, VIEWPORT.x / VIEWPORT.y, radius * 0.05, radius * 10.0),
        false => opengl::orthographic(
            -radius * 1.4,
            radius * 1.4,
            -radius * 1.05,
            radius * 1.05,
            radius * 0.05,
            radius * 10.0,
        ),
    };
    match View::new(
        projection * camera,
        Viewport::from_size(VIEWPORT),
        ClipSpace::OPENGL,
    ) {
        Some(view) => view,
        None => panic!("singular test view"),
    }
}

pub fn screen_point(view: &View, mesh: &EditMesh, rng: &mut Rng) -> Vec2 {
    let verts: Vec<_> = mesh.verts().collect();
    if !verts.is_empty() && rng.chance(0.7) {
        let vert = verts[rng.below(verts.len())];
        let jitter = Vec2::new(rng.range(-12.0, 12.0), rng.range(-12.0, 12.0));
        if let Some(projected) = view.project(mesh.position(vert)) {
            return projected.position + jitter;
        }
    }
    Vec2::new(rng.range(0.0, VIEWPORT.x), rng.range(0.0, VIEWPORT.y))
}
