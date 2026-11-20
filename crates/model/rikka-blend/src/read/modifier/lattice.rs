use rikka_mesh_modifiers::{Lattice, LatticeGrid, LatticeInterpolation};
use rikka_model_core::glam::{Mat4, Vec3};

use crate::file::Record;
use crate::read::scene::world_matrix;
use crate::scene::dna::OB_LATTICE;
use crate::write::Struct;

const INVERT_GROUP: i64 = 1;
const INTERPOLATIONS: [LatticeInterpolation; 4] = [
    LatticeInterpolation::Linear,
    LatticeInterpolation::Cardinal,
    LatticeInterpolation::BSpline,
    LatticeInterpolation::CatmullRom,
];

pub(super) struct Parsed {
    pub(super) lattice: Lattice,
    pub(super) note: Option<String>,
}

pub(super) fn read(modifier: &Record<'_>, owner: &Record<'_>) -> Parsed {
    let flag = modifier.int("flag").unwrap_or(0);
    let mut lattice = Lattice {
        lattice: LatticeGrid::default(),
        strength: modifier
            .float("strength")
            .filter(|value| value.is_finite())
            .unwrap_or(1.0),
        vertex_group: modifier.text("name").unwrap_or_default(),
        invert_vertex_group: flag & INVERT_GROUP != 0,
    };
    let object = modifier
        .id("object")
        .filter(|object| object.int("type") == Some(OB_LATTICE));
    let grid = object.and_then(|object| {
        let data = object.id("data")?;
        let relative = world_matrix(*owner).inverse() * world_matrix(object);
        grid(&data, relative)
    });
    let note = match grid {
        Some(grid) => {
            lattice.lattice = grid;
            None
        }
        None => Some("lattice object missing, using an undeformed lattice".to_owned()),
    };
    Parsed { lattice, note }
}

fn grid(data: &Record<'_>, relative: Mat4) -> Option<LatticeGrid> {
    let count = |name: &str| u16::try_from(data.int(name).unwrap_or(1)).ok();
    let resolution = [count("pntsu")?, count("pntsv")?, count("pntsw")?];
    let total: usize = resolution.iter().map(|&count| count as usize).product();
    let points: Vec<Vec3> = data
        .table("def", "BPoint")?
        .floats::<4>("vec")?
        .into_iter()
        .map(|[x, y, z, _]| Vec3::new(x, y, z))
        .collect();
    if points.len() != total {
        return None;
    }
    let names = [("fu", "du"), ("fv", "dv"), ("fw", "dw")];
    let mut scale = Vec3::ONE;
    let mut offset = Vec3::ZERO;
    for (axis, (start, step)) in names.into_iter().enumerate() {
        if resolution[axis] > 1 {
            let span = data.float(step).unwrap_or(0.0) * f32::from(resolution[axis] - 1);
            if span == 0.0 || !span.is_finite() {
                return None;
            }
            scale[axis] = span;
            offset[axis] = data.float(start).unwrap_or(-0.5) + 0.5 * span;
        }
    }
    let to_file = Mat4::from_translation(offset) * Mat4::from_scale(scale);
    let to_grid = to_file.inverse();
    let points = points
        .into_iter()
        .map(|point| to_grid.transform_point3(point))
        .collect();
    let mut grid = LatticeGrid::from_points(resolution, relative * to_file, points).ok()?;
    let kind = |name: &str| {
        usize::try_from(data.int(name).unwrap_or(2))
            .ok()
            .and_then(|index| INTERPOLATIONS.get(index).copied())
            .unwrap_or_default()
    };
    grid.interpolation = [kind("typeu"), kind("typev"), kind("typew")];
    Some(grid)
}

pub(super) fn write(lattice: &Lattice, record: &mut Struct<'_>) {
    record
        .set_float("strength", lattice.strength)
        .set_text("name", &lattice.vertex_group)
        .set_flag("flag", INVERT_GROUP, lattice.invert_vertex_group);
}

pub(crate) fn interpolation_code(interpolation: LatticeInterpolation) -> i64 {
    INTERPOLATIONS
        .iter()
        .position(|known| *known == interpolation)
        .unwrap_or(2) as i64
}
