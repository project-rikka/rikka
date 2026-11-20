use glam::{Vec3, Vec4};
use rikka_gpu::ComputeBuffer;
use rikka_model_core::Instance;
use rikka_physics_cloth::{Cloth, DRAPE_VERTEX_BYTES, Drape, FigureId, Inputs};
use rikka_physics_tailor::EdgeKind;

use super::device::compute;
use super::sheet::Sheet;

pub(crate) fn step(cloth: &mut Cloth, figure: FigureId, instance: &Instance, frame: u32) {
    let inputs = Inputs::new().figure(figure, instance);
    cloth
        .advance(f64::from(frame) / f64::from(cloth.rate()), &inputs)
        .expect("a step");
}

pub(crate) fn positions(cloth: &mut Cloth, figure: FigureId) -> Vec<Vec3> {
    cloth
        .read_particles(figure)
        .expect("particles")
        .iter()
        .map(|particle| particle.position)
        .collect()
}

pub(crate) fn bits(cloth: &mut Cloth, figure: FigureId) -> Vec<u32> {
    cloth
        .read_particles(figure)
        .expect("particles")
        .iter()
        .flat_map(|particle| {
            particle
                .position
                .to_array()
                .into_iter()
                .chain(particle.velocity.to_array())
        })
        .map(f32::to_bits)
        .collect()
}

pub(crate) fn read(buffer: &ComputeBuffer, bytes: u64) -> Vec<u8> {
    let mut batch = compute().batch();
    let download = batch.download_range(buffer, 0, bytes);
    batch.submit().expect("a readback");
    download.wait().expect("readback bytes")
}

pub(crate) struct DrapedVertex {
    pub(crate) position: Vec3,
    pub(crate) blend: f32,
    pub(crate) normal: Vec3,
    pub(crate) tangent: Vec4,
}

pub(crate) fn draped(drape: &Drape) -> Vec<DrapedVertex> {
    drape.ready.wait().expect("the drape is made");
    let bytes = read(
        &drape.buffer,
        u64::from(drape.vertices) * DRAPE_VERTEX_BYTES,
    );
    bytes
        .as_chunks::<48>()
        .0
        .iter()
        .map(|vertex| {
            let floats: Vec<f32> = vertex
                .as_chunks::<4>()
                .0
                .iter()
                .map(|word| f32::from_le_bytes(*word))
                .collect();
            DrapedVertex {
                position: Vec3::from_slice(&floats[0..3]),
                blend: floats[3],
                normal: Vec3::from_slice(&floats[4..7]),
                tangent: Vec4::from_slice(&floats[8..12]),
            }
        })
        .collect()
}

pub(crate) fn longest_stretch(sheet: &Sheet, positions: &[Vec3]) -> f32 {
    sheet.asset.pieces[0]
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Structural)
        .map(|edge| {
            positions[edge.ends[0] as usize].distance(positions[edge.ends[1] as usize]) / edge.rest
        })
        .fold(0.0, f32::max)
}
