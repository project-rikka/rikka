use std::f32::consts::TAU;

use rikka_physics_math::{Vec3, cos_sin};

use super::data::{HEIGHT_FIELD_HOLE, HeightField};
use super::def::HeightFieldDef;
use crate::Result;

const GLOBAL_HEIGHT: f32 = 256.0;

fn hole_pattern(rows: u32, columns: u32, holes: bool) -> Vec<u8> {
    let cells = rows.saturating_sub(1) * columns.saturating_sub(1);
    (0..cells)
        .map(|cell| {
            if holes && cell > 0 && cell % 16 == 0 {
                HEIGHT_FIELD_HOLE
            } else {
                0
            }
        })
        .collect()
}

impl HeightField {
    pub fn grid(rows: u32, columns: u32, scale: Vec3, holes: bool) -> Result<Self> {
        let heights = vec![0.0; (rows * columns) as usize];
        let materials = hole_pattern(rows, columns, holes);
        Self::build(
            &HeightFieldDef::new(&heights, columns, rows)
                .scale(scale)
                .materials(&materials)
                .height_range(-GLOBAL_HEIGHT, GLOBAL_HEIGHT),
        )
    }

    pub fn wave(
        rows: u32,
        columns: u32,
        scale: Vec3,
        row_frequency: f32,
        column_frequency: f32,
        holes: bool,
    ) -> Result<Self> {
        let omega_z = TAU * row_frequency;
        let omega_x = TAU * column_frequency;
        let heights: Vec<f32> = (0..rows)
            .flat_map(|row| {
                let row_height = cos_sin(omega_z * row as f32).sine;
                (0..columns).map(move |column| row_height * cos_sin(omega_x * column as f32).sine)
            })
            .collect();
        let materials = hole_pattern(rows, columns, holes);
        Self::build(
            &HeightFieldDef::new(&heights, columns, rows)
                .scale(scale)
                .materials(&materials)
                .height_range(-GLOBAL_HEIGHT, GLOBAL_HEIGHT),
        )
    }
}
