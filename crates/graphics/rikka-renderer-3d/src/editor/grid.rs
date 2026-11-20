use bytemuck::{Pod, Zeroable};
use glam::Vec3;

use crate::camera::Camera;

pub const GRID_STEPS: usize = 8;
pub(crate) const GRID_STEPS_DRAWN: u32 = 3;
pub(crate) const GRID_ITERATIONS: u32 = 4;

const PERSPECTIVE_LINES: u32 = 151;
const ORTHOGRAPHIC_LINES: u32 = 301;
const ALIGNED_LINE_LIMIT: u32 = 8192;
const ALIGNED_ZOOM: f32 = 10.0 * 12.0;
const CORNER_VERTICES: u32 = 6;
const AXIS_LINES: u32 = 3;
const SNAP_EPSILON: f32 = 1e-6;

const SHOW_GRID: u32 = 1 << 0;
const SHOW_AXES: u32 = 1 << 1;
const AXIS_X: u32 = 1 << 2;
const AXIS_Y: u32 = 1 << 3;
const AXIS_Z: u32 = 1 << 4;
const ALIGNED: u32 = 1 << 5;
const BEHIND_GEOMETRY: u32 = 1 << 6;
const PERSPECTIVE: u32 = 1 << 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GridPlane {
    Floor,
    Front,
    Side,
}

impl GridPlane {
    fn axes(self) -> (usize, usize) {
        match self {
            Self::Floor => (0, 2),
            Self::Front => (0, 1),
            Self::Side => (2, 1),
        }
    }

    fn normal(self) -> usize {
        match self {
            Self::Floor => 1,
            Self::Front => 2,
            Self::Side => 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridColors {
    pub line: [u8; 4],
    pub emphasis: [u8; 4],
    pub axes: [[u8; 4]; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EditorGrid {
    pub plane: GridPlane,
    pub show_plane: bool,
    pub axes: [bool; 3],
    pub aligned: bool,
    pub steps: [f32; GRID_STEPS],
    pub distance: f32,
    pub clip_end: f32,
    pub line_width: f32,
    pub colors: GridColors,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct GpuGrid {
    pub steps: [f32; GRID_STEPS],
    pub offset: [f32; 2],
    pub clip: f32,
    pub level: f32,
    pub lines: u32,
    pub grid_flags: u32,
    pub axis_flags: u32,
    pub pixel_fac: f32,
    pub axes: [u32; 4],
    pub colors: [u32; 5],
    pub far_clip: f32,
    pub width: f32,
    pub pad: u32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct GridDraws {
    pub grid_vertices: u32,
    pub axis_vertices: u32,
    pub iterations: u32,
}

impl EditorGrid {
    pub(crate) fn resolve(&self, camera: &Camera, width: u32) -> Option<(GpuGrid, GridDraws)> {
        let perspective = camera.projection.w_axis.w < 0.5;
        let axis_bits = [AXIS_X, AXIS_Y, AXIS_Z];
        let shown_axes = self
            .axes
            .iter()
            .zip(axis_bits)
            .filter(|(shown, _)| **shown)
            .fold(0, |bits, (_, bit)| bits | bit);
        let (u, v) = self.plane.axes();
        let mut axis_flags = match self.aligned {
            true => shown_axes & (axis_bits[u] | axis_bits[v]),
            false => shown_axes,
        };
        let mut grid_flags = match self.show_plane {
            true => SHOW_GRID | shown_axes,
            false => 0,
        };
        if axis_flags != 0 {
            axis_flags |= SHOW_AXES;
        }
        if self.aligned {
            axis_flags |= ALIGNED | BEHIND_GEOMETRY;
            grid_flags |= ALIGNED | BEHIND_GEOMETRY;
            if !self.show_plane {
                grid_flags = 0;
            }
            if axis_flags & (AXIS_X | AXIS_Y | AXIS_Z) == 0 {
                axis_flags = 0;
            }
        }
        if axis_flags & SHOW_AXES == 0 {
            axis_flags = 0;
        }
        if grid_flags == 0 && axis_flags == 0 {
            return None;
        }
        if perspective {
            grid_flags |= PERSPECTIVE;
            axis_flags |= PERSPECTIVE;
        }

        let mut steps = self.steps;
        for index in 1..GRID_STEPS {
            if steps[index] == 0.0 || steps[index] == steps[index - 1] {
                steps[index] = steps[index - 1] * 10.0;
            }
        }
        let to_world = camera.view.inverse();
        let position = to_world.w_axis.truncate();
        let backward = to_world.z_axis.truncate().normalize_or(Vec3::Z);
        let up = self.plane.normal();
        let half = Vec3::new(
            1.0 / camera.projection.x_axis.x.abs().max(f32::MIN_POSITIVE),
            1.0 / camera.projection.y_axis.y.abs().max(f32::MIN_POSITIVE),
            0.0,
        );
        let distance = match (perspective, self.aligned) {
            (true, _) => {
                let along = (position[up] / backward[up].abs().max(SNAP_EPSILON)).abs();
                let below = position[up].abs();
                along + (below - along) * (1.0 - backward[up].abs())
            }
            (false, true) => (ALIGNED_ZOOM * half.x / width.max(1) as f32).max(steps[0]),
            (false, false) => self.distance,
        };
        let center = match (perspective, self.aligned) {
            (_, true) => position,
            (true, false) => position - distance * backward,
            (false, false) if backward[up].abs() > SNAP_EPSILON => {
                position - backward * (position[up] / backward[up])
            }
            (false, false) => position,
        };
        let mut level = (GRID_STEPS - 1) as f32;
        for index in 0..GRID_STEPS {
            let current = steps[index];
            let next = match index + 1 < GRID_STEPS {
                true => steps[index + 1],
                false => current * 10.0,
            };
            if next >= distance || index == GRID_STEPS - 1 {
                level = index as f32 + (distance - current) / (next - current);
                break;
            }
        }
        let clip = match perspective {
            true => self.clip_end,
            false => 4.0 * half.x.max(half.y),
        };
        let (lines, iterations) = match (perspective, self.aligned) {
            (true, _) => (PERSPECTIVE_LINES, GRID_ITERATIONS),
            (false, true) => {
                let lowest = steps[(level.max(0.0) as usize).min(GRID_STEPS - 1)];
                let across = |extent: f32| 2 * (extent / lowest).ceil() as u32 + 1;
                (
                    across(half.x).max(across(half.y)).min(ALIGNED_LINE_LIMIT),
                    1,
                )
            }
            (false, false) => (ORTHOGRAPHIC_LINES, 1),
        };
        let pixel_fac = match perspective {
            true => 0.0,
            false => 2.0 * half.x / width.max(1) as f32,
        };
        let packed = |color: [u8; 4]| u32::from_le_bytes(color);
        let gpu = GpuGrid {
            steps,
            offset: [center[u], center[v]],
            clip,
            level,
            lines,
            grid_flags,
            axis_flags,
            pixel_fac,
            axes: [u as u32, v as u32, up as u32, 0],
            colors: [
                packed(self.colors.line),
                packed(self.colors.emphasis),
                packed(self.colors.axes[0]),
                packed(self.colors.axes[1]),
                packed(self.colors.axes[2]),
            ],
            far_clip: self.clip_end,
            width: self.line_width,
            pad: 0,
        };
        let draws = GridDraws {
            grid_vertices: match grid_flags {
                0 => 0,
                _ => CORNER_VERTICES * 2 * GRID_STEPS_DRAWN * lines,
            },
            axis_vertices: match axis_flags {
                0 => 0,
                _ => CORNER_VERTICES * AXIS_LINES,
            },
            iterations,
        };
        Some((gpu, draws))
    }
}
