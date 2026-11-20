use rikka_physics_core::Section;
use rikka_physics_math::{Aabb, Vec3};

use super::data::{
    BLOCK_SHIFT, HEIGHT_FIELD_HOLE, HeightField, HeightFieldHeader, HeightFieldParts, LEVEL_SHIFT,
    Level,
};
use super::def::HeightFieldDef;
use crate::triangulated::triangle::{EdgeFlags, signed_volume};
use crate::{Error, Result, Tolerance};

const COS_FLAT: f32 = 0.9962;
const EMPTY_RANGE: [u16; 2] = [u16::MAX, 0];

impl HeightField {
    pub fn build(def: &HeightFieldDef<'_>) -> Result<Self> {
        validate(def)?;
        let (columns, rows) = (def.columns, def.rows);
        let (min_height, max_height) = def.height_range.unwrap_or_else(|| {
            def.heights
                .iter()
                .fold((f32::MAX, -f32::MAX), |(low, high), &h| {
                    (low.min(h), high.max(h))
                })
        });
        let span = (max_height - min_height).max(Tolerance::default().linear_slop());
        let height_scale = span / f32::from(u16::MAX);
        let heights: Vec<u16> = def
            .heights
            .iter()
            .map(|&h| {
                ((h.clamp(min_height, max_height) - min_height) / height_scale)
                    .round()
                    .clamp(0.0, f32::from(u16::MAX)) as u16
            })
            .collect();
        let cells = ((columns - 1) * (rows - 1)) as usize;
        let materials = if def.materials.is_empty() {
            vec![0; cells]
        } else {
            def.materials.to_vec()
        };
        let mut header = HeightFieldHeader {
            bounds: Aabb::EMPTY,
            scale: def.scale,
            min_height,
            height_scale,
            columns,
            rows,
            clockwise: u32::from(def.clockwise),
            root: 0,
            heights: Section::EMPTY,
            materials: Section::EMPTY,
            flags: Section::EMPTY,
            ranges: Section::EMPTY,
            levels: Section::EMPTY,
        };
        let decode = |q: u16| def.scale.y * (min_height + height_scale * f32::from(q));
        let vertex = |column: u32, row: u32| {
            Vec3::new(
                def.scale.x * column as f32,
                decode(heights[(row * columns + column) as usize]),
                def.scale.z * row as f32,
            )
        };
        let flags = edge_flags(columns, rows, &materials, &vertex);
        let (ranges, levels) = hierarchy(columns, rows, &heights, &materials);
        let root = &ranges[levels.last().map_or(0, |level| level.offset) as usize];
        header.root = (levels.len() as u32 - 1) << LEVEL_SHIFT;
        header.bounds = if root[0] <= root[1] {
            Aabb::new(
                Vec3::new(0.0, decode(root[0]), 0.0),
                Vec3::new(
                    def.scale.x * (columns - 1) as f32,
                    decode(root[1]),
                    def.scale.z * (rows - 1) as f32,
                ),
            )
        } else {
            Aabb::new(Vec3::ZERO, Vec3::ZERO)
        };
        Ok(Self::assemble(HeightFieldParts {
            header,
            heights: &heights,
            materials: &materials,
            flags: &flags,
            ranges: &ranges,
            levels: &levels,
        }))
    }
}

fn validate(def: &HeightFieldDef<'_>) -> Result<()> {
    if def.columns < 2 || def.rows < 2 {
        return Err(Error::HeightFieldTooSmall {
            rows: def.rows,
            columns: def.columns,
        });
    }
    let expected = def.columns as usize * def.rows as usize;
    if def.heights.len() != expected {
        return Err(Error::HeightFieldSize {
            expected,
            actual: def.heights.len(),
        });
    }
    let cells = (def.columns as usize - 1) * (def.rows as usize - 1);
    if !def.materials.is_empty() && def.materials.len() != cells {
        return Err(Error::MaterialCount {
            expected: cells,
            actual: def.materials.len(),
        });
    }
    for (what, value) in [
        ("height field scale", def.scale.x),
        ("height field scale", def.scale.y),
        ("height field scale", def.scale.z),
    ] {
        if !(value.is_finite() && value > 0.0) {
            return Err(Error::InvalidDimension { what, value });
        }
    }
    if !def.heights.iter().all(|h| h.is_finite()) {
        return Err(Error::NonFinite {
            what: "height field height",
        });
    }
    Ok(())
}

fn edge_flags(
    columns: u32,
    rows: u32,
    materials: &[u8],
    vertex: &impl Fn(u32, u32) -> Vec3,
) -> Vec<u8> {
    let cells_x = columns - 1;
    let cells_z = rows - 1;
    let solid = |column: i64, row: i64| {
        column >= 0
            && row >= 0
            && column < i64::from(cells_x)
            && row < i64::from(cells_z)
            && materials[(row as u32 * cells_x + column as u32) as usize] != HEIGHT_FIELD_HOLE
    };
    let point = |column: i64, row: i64| vertex(column as u32, row as u32);
    let mut flags = vec![0u8; 2 * materials.len()];
    for row in 0..i64::from(cells_z) {
        for column in 0..i64::from(cells_x) {
            if !solid(column, row) {
                continue;
            }
            let p11 = point(column, row);
            let p12 = point(column + 1, row);
            let p21 = point(column, row + 1);
            let p22 = point(column + 1, row + 1);
            let first = [p11, p21, p12];
            let second = [p22, p12, p21];
            let mut first_flags = edge_flag(first, p22, normal(second));
            first_flags = EdgeFlags(first_flags.0 << 1);
            let mut second_flags = edge_flag(second, p11, normal(first));
            second_flags = EdgeFlags(second_flags.0 << 1);
            if solid(column - 1, row) {
                let neighbor = [p21, p11, point(column - 1, row + 1)];
                first_flags |= edge_flag(first, neighbor[2], normal(neighbor));
            }
            if solid(column, row - 1) {
                let neighbor = [p12, point(column + 1, row - 1), p11];
                first_flags |= EdgeFlags(edge_flag(first, neighbor[1], normal(neighbor)).0 << 2);
            }
            if solid(column + 1, row) {
                let neighbor = [p12, p22, point(column + 2, row)];
                second_flags |= edge_flag(second, neighbor[2], normal(neighbor));
            }
            if solid(column, row + 1) {
                let neighbor = [p21, point(column, row + 2), p22];
                second_flags |= EdgeFlags(edge_flag(second, neighbor[1], normal(neighbor)).0 << 2);
            }
            let cell = (row as u32 * cells_x + column as u32) as usize;
            flags[2 * cell] = first_flags.0;
            flags[2 * cell + 1] = second_flags.0;
        }
    }
    flags
}

#[inline]
fn normal([a, b, c]: [Vec3; 3]) -> Vec3 {
    (b - a).cross(c - a).normalize_or_zero()
}

#[inline]
fn edge_flag(triangle: [Vec3; 3], opposite: Vec3, neighbor_normal: Vec3) -> EdgeFlags {
    let [a, b, c] = triangle;
    let volume = signed_volume(a, b, c, opposite);
    let flat = normal(triangle).dot(neighbor_normal) > COS_FLAT;
    let mut flags = EdgeFlags::NONE;
    if volume > 0.0 || flat {
        flags |= EdgeFlags::concave(0);
    }
    if volume < 0.0 || flat {
        flags |= EdgeFlags::inverse_concave(0);
    }
    flags
}

fn hierarchy(
    columns: u32,
    rows: u32,
    heights: &[u16],
    materials: &[u8],
) -> (Vec<[u16; 2]>, Vec<Level>) {
    let cells_x = columns - 1;
    let cells_z = rows - 1;
    let block = 1u32 << BLOCK_SHIFT;
    let mut levels = vec![Level {
        offset: 0,
        width: cells_x.div_ceil(block),
        depth: cells_z.div_ceil(block),
        shift: BLOCK_SHIFT,
    }];
    let mut ranges = Vec::new();
    for bz in 0..levels[0].depth {
        for bx in 0..levels[0].width {
            let mut range = EMPTY_RANGE;
            for row in bz * block..((bz + 1) * block).min(cells_z) {
                for column in bx * block..((bx + 1) * block).min(cells_x) {
                    if materials[(row * cells_x + column) as usize] == HEIGHT_FIELD_HOLE {
                        continue;
                    }
                    for (dx, dz) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let h = heights[((row + dz) * columns + column + dx) as usize];
                        range = [range[0].min(h), range[1].max(h)];
                    }
                }
            }
            ranges.push(range);
        }
    }
    loop {
        let below = levels[levels.len() - 1];
        if below.width == 1 && below.depth == 1 && levels.len() > 1 {
            break;
        }
        let level = Level {
            offset: ranges.len() as u32,
            width: below.width.div_ceil(2),
            depth: below.depth.div_ceil(2),
            shift: below.shift + 1,
        };
        for z in 0..level.depth {
            for x in 0..level.width {
                let mut range = EMPTY_RANGE;
                for (dx, dz) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let (cx, cz) = (2 * x + dx, 2 * z + dz);
                    if cx < below.width && cz < below.depth {
                        let child = ranges[(below.offset + cz * below.width + cx) as usize];
                        range = [range[0].min(child[0]), range[1].max(child[1])];
                    }
                }
                ranges.push(range);
            }
        }
        levels.push(level);
    }
    (ranges, levels)
}
