use std::f32::consts::TAU;

use glam::Vec3;
use rikka_physics_math::cos_sin;

use crate::blueprint::Geometry;
use crate::error::Result;

pub(crate) fn cylinder(height: f32, radius: f32, y_offset: f32, sides: usize) -> Result<Geometry> {
    let mut points = Vec::with_capacity(2 * sides);
    for (x, z) in ring(radius, sides) {
        points.push(Vec3::new(x, y_offset, z));
        points.push(Vec3::new(x, y_offset + height, z));
    }
    Geometry::hull(points)
}

pub(crate) fn rock(radius: f32) -> Result<Geometry> {
    const COUNT: usize = 10;
    let phi = 0.5 * (1.0 + 5.0f32.sqrt());
    let step = cos_sin(TAU / phi);
    let mut angle = (1.0f32, 0.0f32);
    let mut points = Vec::with_capacity(COUNT);
    for index in 0..COUNT {
        let z = 1.0 - (2.0 * index as f32 + 1.0) / COUNT as f32;
        let planar = (1.0 - z * z).sqrt();
        points.push(radius * Vec3::new(planar * angle.0, planar * angle.1, z));
        angle = (
            step.cosine * angle.0 - step.sine * angle.1,
            step.sine * angle.0 + step.cosine * angle.1,
        );
    }
    Geometry::hull(points)
}

pub(crate) fn wave(
    x_count: u32,
    z_count: u32,
    cell_width: f32,
    amplitude: f32,
    row_frequency: f32,
    column_frequency: f32,
) -> Geometry {
    let omega_z = TAU * row_frequency * cell_width;
    let omega_x = TAU * column_frequency * cell_width;
    let x0 = -0.5 * cell_width * x_count as f32;
    let z0 = -0.5 * cell_width * z_count as f32;
    let mut vertices = Vec::with_capacity(((x_count + 1) * (z_count + 1)) as usize);
    for ix in 0..=x_count {
        for iz in 0..=z_count {
            let height =
                amplitude * cos_sin(omega_x * ix as f32).sine * cos_sin(omega_z * iz as f32).sine;
            vertices.push(Vec3::new(
                x0 + cell_width * ix as f32,
                height,
                z0 + cell_width * iz as f32,
            ));
        }
    }
    let mut indices = Vec::with_capacity(6 * (x_count * z_count) as usize);
    for ix in 0..x_count {
        for iz in 0..z_count {
            let index1 = iz + (z_count + 1) * ix;
            let index2 = index1 + 1;
            let index3 = index2 + z_count + 1;
            let index4 = index3 - 1;
            indices.extend_from_slice(&[index1, index2, index3, index3, index4, index1]);
        }
    }
    Geometry::mesh(vertices, indices)
}

pub(crate) fn inward_cylinder(
    segments: u32,
    radius: f32,
    half_depth: f32,
    y_center: f32,
) -> Geometry {
    let mut vertices = Vec::with_capacity(2 * segments as usize + 2);
    for i in 0..segments {
        let angle = cos_sin(TAU * i as f32 / segments as f32);
        let (x, y) = (radius * angle.cosine, radius * angle.sine + y_center);
        vertices.push(Vec3::new(x, y, -half_depth));
        vertices.push(Vec3::new(x, y, half_depth));
    }
    let near = 2 * segments;
    let far = near + 1;
    vertices.push(Vec3::new(0.0, y_center, -half_depth));
    vertices.push(Vec3::new(0.0, y_center, half_depth));
    let mut indices = Vec::with_capacity(12 * segments as usize);
    for i in 0..segments {
        let j = (i + 1) % segments;
        indices.extend_from_slice(&[2 * i, 2 * i + 1, 2 * j + 1]);
        indices.extend_from_slice(&[2 * i, 2 * j + 1, 2 * j]);
        indices.extend_from_slice(&[near, 2 * i, 2 * j]);
        indices.extend_from_slice(&[far, 2 * j + 1, 2 * i + 1]);
    }
    Geometry::mesh(vertices, indices)
}

fn ring(radius: f32, sides: usize) -> impl Iterator<Item = (f32, f32)> {
    (0..sides).map(move |index| {
        let angle = cos_sin(TAU * index as f32 / sides as f32);
        (radius * angle.cosine, radius * angle.sine)
    })
}
