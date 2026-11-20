use std::f32::consts::TAU;

use rikka_physics_math::{Vec3, cos_sin};

use super::data::TriangleMesh;
use super::def::MeshDef;
use crate::Result;

const BOX_TRIANGLES: [u32; 36] = [
    0, 1, 3, 1, 2, 3, 0, 4, 1, 1, 4, 5, 0, 3, 7, 4, 0, 7, 4, 7, 5, 6, 5, 7, 1, 5, 2, 6, 2, 5, 3, 2,
    7, 6, 7, 2,
];

const HOLLOW_BOX_TRIANGLES: [u32; 36] = [
    3, 1, 0, 3, 2, 1, 1, 4, 0, 5, 4, 1, 7, 3, 0, 7, 0, 4, 5, 7, 4, 7, 5, 6, 2, 5, 1, 5, 2, 6, 7, 2,
    3, 2, 7, 6,
];

fn build(def: MeshDef<'_>) -> Result<TriangleMesh> {
    TriangleMesh::build(&def).map(|build| build.mesh)
}

fn grid_indices(x_count: u32, z_count: u32) -> Vec<u32> {
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
    indices
}

fn grid_vertices(
    x_count: u32,
    z_count: u32,
    cell_width: f32,
    height: impl Fn(u32, u32) -> f32,
) -> Vec<Vec3> {
    let x0 = -0.5 * cell_width * x_count as f32;
    let z0 = -0.5 * cell_width * z_count as f32;
    let mut vertices = Vec::with_capacity(((x_count + 1) * (z_count + 1)) as usize);
    for ix in 0..=x_count {
        for iz in 0..=z_count {
            vertices.push(Vec3::new(
                x0 + cell_width * ix as f32,
                height(ix, iz),
                z0 + cell_width * iz as f32,
            ));
        }
    }
    vertices
}

impl TriangleMesh {
    pub fn grid(x_count: u32, z_count: u32, cell_width: f32, material_count: u8) -> Result<Self> {
        let vertices = grid_vertices(x_count, z_count, cell_width, |_, _| 0.0);
        let indices = grid_indices(x_count, z_count);
        let materials: Vec<u8> = if material_count > 0 {
            (0..x_count * z_count)
                .flat_map(|cell| {
                    let material = (cell % u32::from(material_count)) as u8;
                    [material, material]
                })
                .collect()
        } else {
            Vec::new()
        };
        build(MeshDef::new(&vertices, &indices).materials(&materials))
    }

    pub fn wave(
        x_count: u32,
        z_count: u32,
        cell_width: f32,
        amplitude: f32,
        row_frequency: f32,
        column_frequency: f32,
    ) -> Result<Self> {
        let omega_z = TAU * row_frequency * cell_width;
        let omega_x = TAU * column_frequency * cell_width;
        let vertices = grid_vertices(x_count, z_count, cell_width, |ix, iz| {
            amplitude * cos_sin(omega_x * ix as f32).sine * cos_sin(omega_z * iz as f32).sine
        });
        let indices = grid_indices(x_count, z_count);
        build(MeshDef::new(&vertices, &indices))
    }

    pub fn torus(radial: u32, tubular: u32, radius: f32, thickness: f32) -> Result<Self> {
        let mut vertices = Vec::with_capacity((radial * tubular) as usize);
        for radial_index in 0..radial {
            for tubular_index in 0..tubular {
                let u = tubular_index as f32 / tubular as f32 * TAU;
                let v = radial_index as f32 / radial as f32 * TAU;
                let tube = cos_sin(v);
                let ring = cos_sin(u);
                let distance = radius + thickness * tube.cosine;
                vertices.push(Vec3::new(
                    distance * ring.cosine,
                    distance * ring.sine,
                    thickness * tube.sine,
                ));
            }
        }
        let mut indices = Vec::with_capacity(6 * vertices.len());
        for radial1 in 0..radial {
            let radial2 = (radial1 + 1) % radial;
            for tubular1 in 0..tubular {
                let tubular2 = (tubular1 + 1) % tubular;
                let index1 = radial1 * tubular + tubular1;
                let index2 = radial1 * tubular + tubular2;
                let index3 = radial2 * tubular + tubular2;
                let index4 = radial2 * tubular + tubular1;
                indices.extend_from_slice(&[index1, index2, index3, index3, index4, index1]);
            }
        }
        build(MeshDef::new(&vertices, &indices))
    }

    pub fn cuboid(center: Vec3, extent: Vec3) -> Result<Self> {
        let vertices = box_corners(center, extent, extent);
        build(MeshDef::new(&vertices, &BOX_TRIANGLES))
    }

    pub fn hollow_cuboid(center: Vec3, extent: Vec3) -> Result<Self> {
        let vertices = box_corners(center, extent, extent);
        build(MeshDef::new(&vertices, &HOLLOW_BOX_TRIANGLES))
    }

    pub fn platform(center: Vec3, height: f32, top_width: f32, bottom_width: f32) -> Result<Self> {
        let half_height = 0.5 * height;
        let top = Vec3::new(0.5 * top_width, half_height, 0.5 * top_width);
        let bottom = Vec3::new(0.5 * bottom_width, half_height, 0.5 * bottom_width);
        let vertices = box_corners(center, top, bottom);
        build(MeshDef::new(&vertices, &BOX_TRIANGLES))
    }
}

fn box_corners(center: Vec3, top: Vec3, bottom: Vec3) -> [Vec3; 8] {
    [
        Vec3::new(top.x, top.y, top.z),
        Vec3::new(-top.x, top.y, top.z),
        Vec3::new(-bottom.x, -bottom.y, bottom.z),
        Vec3::new(bottom.x, -bottom.y, bottom.z),
        Vec3::new(top.x, top.y, -top.z),
        Vec3::new(-top.x, top.y, -top.z),
        Vec3::new(-bottom.x, -bottom.y, -bottom.z),
        Vec3::new(bottom.x, -bottom.y, -bottom.z),
    ]
    .map(|corner| corner + center)
}
