use glam::{Mat4, Vec3};
use rikka_file_format::{Codec, DecodeError, Reader, Writer};

use crate::codec::{finite, record, tagged};

pub const LATTICE_MAX_RESOLUTION: u16 = 64;
pub const LATTICE_MAX_POINTS: usize = 32000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LatticeInterpolation {
    Linear,
    Cardinal,
    #[default]
    BSpline,
    CatmullRom,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LatticeGrid {
    resolution: [u16; 3],
    pub interpolation: [LatticeInterpolation; 3],
    pub transform: Mat4,
    points: Vec<Vec3>,
}

impl LatticeGrid {
    #[must_use]
    pub fn new(resolution: [u16; 3], transform: Mat4) -> LatticeGrid {
        let resolution = clamp_resolution(resolution);
        let mut grid = LatticeGrid {
            resolution,
            interpolation: [LatticeInterpolation::BSpline; 3],
            transform,
            points: Vec::new(),
        };
        grid.points = (0..grid.point_count())
            .map(|index| grid.rest_point(grid.cell(index)))
            .collect();
        grid
    }

    #[must_use]
    pub fn fitted(min: Vec3, max: Vec3, resolution: [u16; 3]) -> LatticeGrid {
        let size = (max - min).max(Vec3::splat(1.0e-4));
        let transform = Mat4::from_translation((min + max) * 0.5) * Mat4::from_scale(size);
        LatticeGrid::new(resolution, transform)
    }

    #[must_use]
    pub fn resolution(&self) -> [u16; 3] {
        self.resolution
    }

    #[must_use]
    pub fn point_count(&self) -> usize {
        self.resolution
            .iter()
            .map(|&count| count as usize)
            .product()
    }

    #[must_use]
    pub fn points(&self) -> &[Vec3] {
        &self.points
    }

    pub fn points_mut(&mut self) -> &mut [Vec3] {
        &mut self.points
    }

    #[must_use]
    pub fn index(&self, [u, v, w]: [u16; 3]) -> usize {
        let [nu, nv, _] = self.resolution.map(usize::from);
        usize::from(w) * nu * nv + usize::from(v) * nu + usize::from(u)
    }

    #[must_use]
    pub fn cell(&self, index: usize) -> [u16; 3] {
        let [nu, nv, _] = self.resolution.map(usize::from);
        [
            (index % nu) as u16,
            (index / nu % nv) as u16,
            (index / (nu * nv)) as u16,
        ]
    }

    #[must_use]
    pub fn rest_point(&self, cell: [u16; 3]) -> Vec3 {
        Vec3::from_array(std::array::from_fn(|axis| {
            let (start, step) = rest_axis(self.resolution[axis]);
            (0..cell[axis]).fold(start, |value, _| value + step)
        }))
    }

    #[must_use]
    pub fn rest_axis(&self, axis: usize) -> (f32, f32) {
        rest_axis(self.resolution[axis])
    }

    #[must_use]
    pub fn is_rest(&self) -> bool {
        self.points
            .iter()
            .enumerate()
            .all(|(index, &point)| point == self.rest_point(self.cell(index)))
    }

    pub fn reset(&mut self) {
        for index in 0..self.points.len() {
            self.points[index] = self.rest_point(self.cell(index));
        }
    }

    #[must_use]
    pub fn resized(&self, resolution: [u16; 3]) -> LatticeGrid {
        let mut grid = LatticeGrid::new(resolution, self.transform);
        grid.interpolation = self.interpolation;
        if self.is_rest() {
            return grid;
        }
        let deform = crate::evaluators::lattice::Deform::new(self);
        let to_lattice = self.transform.inverse();
        for point in &mut grid.points {
            let mut moved = self.transform.transform_point3(*point);
            deform.apply(&mut moved, 1.0);
            *point = to_lattice.transform_point3(moved);
        }
        grid
    }

    pub fn from_points(
        resolution: [u16; 3],
        transform: Mat4,
        points: Vec<Vec3>,
    ) -> Result<LatticeGrid, DecodeError> {
        let mut grid = LatticeGrid {
            resolution,
            interpolation: [LatticeInterpolation::BSpline; 3],
            transform,
            points,
        };
        grid.validate()?;
        grid.points.shrink_to_fit();
        Ok(grid)
    }

    fn validate(&self) -> Result<(), DecodeError> {
        if self
            .resolution
            .iter()
            .any(|&count| count == 0 || count > LATTICE_MAX_RESOLUTION)
            || self.point_count() > LATTICE_MAX_POINTS
        {
            return Err(DecodeError::Malformed("lattice resolution is out of range"));
        }
        if self.points.len() != self.point_count() {
            return Err(DecodeError::Malformed("lattice point count mismatch"));
        }
        if !self.points.iter().all(|point| point.is_finite()) || !self.transform.is_finite() {
            return Err(DecodeError::Malformed("lattice point is not finite"));
        }
        Ok(())
    }
}

impl Default for LatticeGrid {
    fn default() -> LatticeGrid {
        LatticeGrid::new([2, 2, 2], Mat4::IDENTITY)
    }
}

fn rest_axis(count: u16) -> (f32, f32) {
    match count {
        0 | 1 => (0.0, 0.0),
        count => (-0.5, 1.0 / f32::from(count - 1)),
    }
}

fn clamp_resolution(resolution: [u16; 3]) -> [u16; 3] {
    let mut resolution = resolution.map(|count| count.clamp(1, LATTICE_MAX_RESOLUTION));
    while resolution
        .iter()
        .map(|&count| count as usize)
        .product::<usize>()
        > LATTICE_MAX_POINTS
    {
        let [u, v, w] = resolution;
        if u >= v && u >= w {
            resolution[0] -= 1;
        } else if v >= u && v >= w {
            resolution[1] -= 1;
        } else {
            resolution[2] -= 1;
        }
    }
    resolution
}

#[derive(Clone, Debug, PartialEq)]
pub struct Lattice {
    pub lattice: LatticeGrid,
    pub strength: f32,
    pub vertex_group: String,
    pub invert_vertex_group: bool,
}

impl Default for Lattice {
    fn default() -> Lattice {
        Lattice {
            lattice: LatticeGrid::default(),
            strength: 1.0,
            vertex_group: String::new(),
            invert_vertex_group: false,
        }
    }
}

tagged!(LatticeInterpolation, "lattice interpolation" {
    Linear = 0,
    Cardinal = 1,
    BSpline = 2,
    CatmullRom = 3,
});

impl Codec for LatticeGrid {
    type Error = DecodeError;

    fn encode(&self, out: &mut Writer<'_>) {
        out.put(&self.resolution);
        out.put(&self.interpolation);
        out.put(&self.transform);
        out.put(&self.points);
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let resolution = input.get()?;
        let interpolation = input.get()?;
        let transform = input.get()?;
        let points = input.get()?;
        let mut grid = LatticeGrid::from_points(resolution, transform, points)?;
        grid.interpolation = interpolation;
        Ok(grid)
    }
}

record!(Lattice {
    lattice,
    strength => finite("lattice strength is not finite"),
    vertex_group,
    invert_vertex_group,
});
