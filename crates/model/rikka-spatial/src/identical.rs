use std::collections::HashMap;

use glam::Vec3;

#[must_use]
pub fn weld_identical(positions: &[Vec3]) -> Vec<u32> {
    let mut first: HashMap<[u32; 3], u32> = HashMap::with_capacity(positions.len());
    positions
        .iter()
        .enumerate()
        .map(|(index, point)| {
            *first
                .entry(point.to_array().map(f32::to_bits))
                .or_insert(index as u32)
        })
        .collect()
}
