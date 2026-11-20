use glam::{Mat3, Mat4, Vec3};
use rikka_polymesh::Polymesh;

use super::vertex_group::Group;
use crate::kinds::lattice::{Lattice, LatticeGrid, LatticeInterpolation};
use crate::note::SkipReason;
use crate::parallel;

const GRAIN: usize = 1 << 12;

pub(crate) struct Deform<'a> {
    grid: &'a LatticeGrid,
    to_lattice: Mat4,
    offsets: Vec<Vec3>,
    origin: [f32; 3],
    step: [f32; 3],
}

impl<'a> Deform<'a> {
    pub(crate) fn new(grid: &'a LatticeGrid) -> Deform<'a> {
        let to_lattice = grid.transform.inverse();
        let back = Mat3::from_mat4(to_lattice.inverse());
        let offsets = grid
            .points()
            .iter()
            .enumerate()
            .map(|(index, &point)| back * (point - grid.rest_point(grid.cell(index))))
            .collect();
        let axes: [(f32, f32); 3] = std::array::from_fn(|axis| grid.rest_axis(axis));
        Deform {
            grid,
            to_lattice,
            offsets,
            origin: axes.map(|(start, _)| start),
            step: axes.map(|(_, step)| step),
        }
    }

    pub(crate) fn apply(&self, co: &mut Vec3, weight: f32) {
        let local = self.to_lattice.transform_point3(*co);
        let resolution = self.grid.resolution().map(i32::from);
        let mut weights = [[0.0f32; 4]; 3];
        let mut starts = [0i32; 3];
        for axis in 0..3 {
            if resolution[axis] > 1 {
                let value = axis_clamp(
                    (local[axis] - self.origin[axis]) / self.step[axis],
                    resolution[axis],
                );
                let start = value.floor() as i32;
                weights[axis] =
                    position_weights(value - start as f32, self.grid.interpolation[axis]);
                starts[axis] = start;
            } else {
                weights[axis] = [0.0, 1.0, 0.0, 0.0];
            }
        }
        let [nu, nv, nw] = resolution;
        let w_stride = nu * nv;
        let w_max = (nw - 1) * nu * nv;
        let v_max = (nv - 1) * nu;
        let u_max = nu - 1;
        let mut sum = *co;
        for ww in starts[2] - 1..=starts[2] + 2 {
            let w = weight * weights[2][(ww - starts[2] + 1) as usize];
            let w_index = (ww * w_stride).clamp(0, w_max);
            for vv in starts[1] - 1..=starts[1] + 2 {
                let v = w * weights[1][(vv - starts[1] + 1) as usize];
                let v_index = (vv * nu).clamp(0, v_max);
                for uu in starts[0] - 1..=starts[0] + 2 {
                    let u = v * weights[0][(uu - starts[0] + 1) as usize];
                    let index = (w_index + v_index + uu.clamp(0, u_max)) as usize;
                    sum += self.offsets[index] * u;
                }
            }
        }
        *co = sum;
    }
}

fn axis_clamp(value: f32, count: i32) -> f32 {
    if value.is_nan() || value <= -2.0 {
        return -2.0;
    }
    if value >= count as f32 {
        return count as f32;
    }
    value
}

fn position_weights(t: f32, interpolation: LatticeInterpolation) -> [f32; 4] {
    let cardinal = |fc: f32| {
        let t2 = t * t;
        let t3 = t2 * t;
        [
            -fc * t3 + 2.0 * fc * t2 - fc * t,
            (2.0 - fc) * t3 + (fc - 3.0) * t2 + 1.0,
            (fc - 2.0) * t3 + (3.0 - 2.0 * fc) * t2 + fc * t,
            fc * t3 - fc * t2,
        ]
    };
    match interpolation {
        LatticeInterpolation::Linear => [0.0, -t + 1.0, t, 0.0],
        LatticeInterpolation::Cardinal => cardinal(0.71),
        LatticeInterpolation::CatmullRom => cardinal(0.5),
        LatticeInterpolation::BSpline => {
            let t2 = t * t;
            let t3 = t2 * t;
            [
                -0.166_666_66 * t3 + 0.5 * t2 - 0.5 * t + 0.166_666_66,
                0.5 * t3 - t2 + 0.666_666_7,
                -0.5 * t3 + 0.5 * t2 + 0.5 * t + 0.166_666_66,
                0.166_666_66 * t3,
            ]
        }
    }
}

pub(crate) fn evaluate(lattice: &Lattice, mesh: &mut Polymesh) -> Result<(), SkipReason> {
    if lattice.strength == 0.0 {
        return Ok(());
    }
    let group = Group::find(mesh, &lattice.vertex_group);
    let deform = Deform::new(&lattice.lattice);
    let mut positions = std::mem::take(&mut mesh.positions);
    let source = &*mesh;
    parallel::fill(&mut positions, GRAIN, |point, position| {
        let weight = match group {
            Group::Weighted(_) => {
                let weight = group.weight(source, point, lattice.invert_vertex_group);
                if weight <= 0.0 {
                    return;
                }
                weight * lattice.strength
            }
            Group::Unset | Group::Unweighted => lattice.strength,
        };
        deform.apply(position, weight);
    });
    mesh.positions = positions;
    Ok(())
}
