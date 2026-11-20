mod bounds;
mod grid;
mod outline;
mod studio;
mod workbench;

use bytemuck::{Pod, Zeroable};
use glam::{Vec3, Vec4};

use crate::camera::Camera;

pub use bounds::ClipBounds;
pub use grid::{EditorGrid, GRID_STEPS, GridColors, GridPlane};
pub use outline::SelectionOutline;
pub use studio::{STUDIO_LIGHTS, StudioLight, StudioLighting};
pub use workbench::{Cavity, Curvature, StudioShadow, WorkbenchEffects};

pub(crate) use grid::{GRID_ITERATIONS, GpuGrid, GridDraws};
pub(crate) use outline::{OUTLINE_COLORS, OutlinePalette};

pub const CLIP_PLANES: usize = 6;

const PASS_ALL: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct GpuStudioLight {
    pub direction: [f32; 4],
    pub diffuse: [f32; 4],
    pub specular: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct GpuEditor {
    pub clip: [[f32; 4]; CLIP_PLANES],
    pub ambient: [f32; 4],
    pub lights: [GpuStudioLight; STUDIO_LIGHTS],
    pub shadow: [f32; 4],
    pub studio: u32,
    pub shadowed: u32,
    pub shadow_shift: f32,
    pub shadow_focus: f32,
    pub grid: GpuGrid,
}

const _: () = assert!(size_of::<GpuEditor>() == 448);

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Editor {
    pub clip: Option<[Vec4; CLIP_PLANES]>,
    pub bounds: Option<ClipBounds>,
    pub studio: Option<StudioLighting>,
    pub outline: SelectionOutline,
    pub grid: Option<EditorGrid>,
    pub workbench: WorkbenchEffects,
}

pub(crate) struct ResolvedEditor {
    pub gpu: GpuEditor,
    pub grid: Option<GridDraws>,
}

impl Editor {
    pub(crate) fn clips(&self) -> bool {
        self.clip.is_some()
    }

    pub(crate) fn resolve(&self, camera: &Camera, width: u32) -> Option<ResolvedEditor> {
        let grid = self.grid.and_then(|grid| grid.resolve(camera, width));
        let shadow = self.workbench.shadow;
        if self.clip.is_none() && self.studio.is_none() && grid.is_none() && shadow.is_none() {
            return None;
        }
        let mut gpu = GpuEditor {
            clip: [PASS_ALL; CLIP_PLANES],
            ..GpuEditor::default()
        };
        if let Some(planes) = self.clip {
            for (slot, plane) in gpu.clip.iter_mut().zip(planes) {
                *slot = plane.to_array();
            }
        }
        if let Some(studio) = self.studio {
            gpu.studio = 1;
            gpu.ambient = studio.ambient.extend(0.0).to_array();
            for (slot, light) in gpu.lights.iter_mut().zip(studio.lights) {
                *slot = GpuStudioLight {
                    direction: light.direction.normalize_or(Vec3::Z).extend(0.0).to_array(),
                    diffuse: light.diffuse.extend(light.wrap).to_array(),
                    specular: light.specular.extend(0.0).to_array(),
                };
            }
        }
        if let Some(shadow) = shadow {
            gpu.shadowed = 1;
            gpu.shadow = shadow
                .direction
                .normalize_or(Vec3::NEG_Y)
                .extend(shadow.intensity)
                .to_array();
            let focus = shadow.focus.clamp(0.0001, 0.99999);
            gpu.shadow_shift = shadow.shift;
            gpu.shadow_focus = 1.0 - focus * (1.0 - shadow.shift);
        }
        let draws = grid.map(|(resolved, draws)| {
            gpu.grid = resolved;
            draws
        });
        Some(ResolvedEditor { gpu, grid: draws })
    }
}
