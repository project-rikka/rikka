use glam::{Mat4, Vec3, Vec4};

use crate::camera::Camera;
use crate::compute_inputs::ComputeSource;
use crate::drape::Drape;
use crate::editor::{
    CLIP_PLANES, ClipBounds, Editor, EditorGrid, SelectionOutline, StudioLighting, WorkbenchEffects,
};
use crate::fluid::{FluidLayer, Wetness};
use crate::light::{DEFAULT_LIGHT_CHANNELS, Light};
use crate::look::{Figure, FigureId, Look};
use crate::material::MaterialId;
use crate::mesh::MeshId;
use crate::overlay::{OverlayBatch, OverlayData, OverlaySource, Overlays};
use rikka_gpu::compute::{ComputeBuffer, Ticket};

pub const DEFAULT_LAYERS: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SkinRef {
    pub(crate) first: u32,
    pub(crate) count: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OutlineSource {
    #[default]
    Surface,
    Edges,
}

#[derive(Clone, Copy, Debug)]
#[must_use]
pub struct Draw {
    pub mesh: MeshId,
    pub submesh: u32,
    pub material: MaterialId,
    pub transform: Mat4,
    pub object_id: u32,
    pub skin: Option<SkinRef>,
    pub morph_weights: Option<(u32, u32)>,
    pub drape: Option<Drape>,
    pub selected: bool,
    pub visible_to_picking: bool,
    pub order: i32,
    pub figure: Option<FigureId>,
    pub wet: Option<Wetness>,
    pub layers: u32,
    /// Light-linking mask: lights shine on and shadow this draw only when their
    /// [`Light::channels`] share a bit with it.
    pub light_channels: u32,
    pub outline: Option<[u8; 4]>,
    pub outline_only: bool,
    pub outline_source: OutlineSource,
}

impl Draw {
    pub fn new(mesh: MeshId, material: MaterialId) -> Self {
        Self {
            mesh,
            submesh: 0,
            material,
            transform: Mat4::IDENTITY,
            object_id: 0,
            skin: None,
            morph_weights: None,
            drape: None,
            selected: false,
            visible_to_picking: true,
            order: 0,
            figure: None,
            wet: None,
            layers: DEFAULT_LAYERS,
            light_channels: DEFAULT_LIGHT_CHANNELS,
            outline: None,
            outline_only: false,
            outline_source: OutlineSource::Surface,
        }
    }

    pub fn submesh(mut self, submesh: u32) -> Self {
        self.submesh = submesh;
        self
    }

    pub fn transform(mut self, transform: Mat4) -> Self {
        self.transform = transform;
        self
    }

    pub fn object_id(mut self, id: u32) -> Self {
        self.object_id = id;
        self
    }

    pub fn skin(mut self, skin: SkinRef) -> Self {
        self.skin = Some(skin);
        self
    }

    pub fn morph_weights(mut self, first: u32, count: u32) -> Self {
        self.morph_weights = Some((first, count));
        self
    }

    pub fn with_drape(mut self, drape: Drape) -> Self {
        self.drape = Some(drape);
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn pickable(mut self, pickable: bool) -> Self {
        self.visible_to_picking = pickable;
        self
    }

    pub fn order(mut self, order: i32) -> Self {
        self.order = order;
        self
    }

    pub fn figure(mut self, figure: FigureId) -> Self {
        self.figure = Some(figure);
        self
    }

    pub fn light_channels(mut self, channels: u32) -> Self {
        self.light_channels = channels;
        self
    }

    pub fn layers(mut self, layers: u32) -> Self {
        self.layers = layers;
        self
    }

    pub fn wet(mut self, wetness: Wetness) -> Self {
        self.wet = Some(wetness);
        self
    }

    pub fn outlined(mut self, color: [u8; 4]) -> Self {
        self.outline = Some(color);
        self
    }

    pub fn outline_only(mut self, color: [u8; 4]) -> Self {
        self.outline = Some(color);
        self.outline_only = true;
        self
    }

    pub fn outline_source(mut self, source: OutlineSource) -> Self {
        self.outline_source = source;
        self
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Background {
    #[default]
    Transparent,
    Color(Vec3),
    Sky,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ToneMapping {
    #[default]
    Aces,
    Clamp,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
    pub enabled: bool,
    pub color: Vec3,
    pub opacity: f32,
    pub spacing: f32,
    pub reach: f32,
    pub vertical_axis: bool,
}

impl Default for Grid {
    fn default() -> Self {
        Self {
            enabled: false,
            color: Vec3::ONE,
            opacity: 0.35,
            spacing: 1.0,
            reach: 10.0,
            vertical_axis: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Outline {
    pub enabled: bool,
    pub width: u32,
    pub color: [f32; 4],
}

impl Default for Outline {
    fn default() -> Self {
        Self {
            enabled: true,
            width: 2,
            color: [1.0, 0.62, 0.2, 1.0],
        }
    }
}

#[derive(Debug, Default)]
#[must_use]
pub struct Frame {
    pub(crate) camera: Camera,
    pub(crate) draws: Vec<Draw>,
    pub(crate) lights: Vec<Light>,
    pub(crate) bones: Vec<Mat4>,
    pub(crate) morph_weights: Vec<f32>,
    pub(crate) background: Background,
    pub(crate) ambient: Vec3,
    pub(crate) exposure: f32,
    pub(crate) tone_mapping: ToneMapping,
    pub(crate) grid: Grid,
    pub(crate) outline: Outline,
    pub(crate) time: f32,
    pub(crate) look: Option<Look>,
    pub(crate) figures: Vec<Figure>,
    pub(crate) fluids: Vec<FluidLayer>,
    pub(crate) wets: Vec<ComputeSource>,
    pub(crate) drapes: Vec<ComputeSource>,
    pub(crate) cut: bool,
    pub(crate) overlays: Overlays,
    pub(crate) editor: Editor,
}

impl Frame {
    pub fn new(camera: Camera) -> Self {
        Self {
            camera,
            exposure: 1.0,
            ambient: Vec3::splat(0.03),
            ..Self::default()
        }
    }

    pub fn draw(&mut self, draw: Draw) -> &mut Self {
        self.draws.push(draw);
        self
    }

    pub fn draws(&mut self, draws: impl IntoIterator<Item = Draw>) -> &mut Self {
        self.draws.extend(draws);
        self
    }

    pub fn light(&mut self, light: Light) -> &mut Self {
        self.lights.push(light);
        self
    }

    pub fn skin(&mut self, matrices: &[Mat4]) -> SkinRef {
        let first = self.bones.len() as u32;
        self.bones.extend_from_slice(matrices);
        SkinRef {
            first,
            count: matrices.len() as u32,
        }
    }

    pub fn morphs(&mut self, weights: &[f32]) -> (u32, u32) {
        let first = self.morph_weights.len() as u32;
        self.morph_weights.extend_from_slice(weights);
        (first, weights.len() as u32)
    }

    pub fn background(&mut self, background: Background) -> &mut Self {
        self.background = background;
        self
    }

    pub fn ambient(&mut self, ambient: Vec3) -> &mut Self {
        self.ambient = ambient;
        self
    }

    pub fn exposure(&mut self, exposure: f32) -> &mut Self {
        self.exposure = exposure;
        self
    }

    pub fn tone_mapping(&mut self, tone_mapping: ToneMapping) -> &mut Self {
        self.tone_mapping = tone_mapping;
        self
    }

    pub fn grid(&mut self, grid: Grid) -> &mut Self {
        self.grid = grid;
        self
    }

    pub fn outline(&mut self, outline: Outline) -> &mut Self {
        self.outline = outline;
        self
    }

    pub fn selection_outline(&mut self, outline: SelectionOutline) -> &mut Self {
        self.editor.outline = outline;
        self
    }

    pub fn clip_planes(&mut self, planes: [Vec4; CLIP_PLANES]) -> &mut Self {
        self.editor.clip = Some(planes);
        self
    }

    pub fn clip_bounds(&mut self, bounds: ClipBounds) -> &mut Self {
        self.editor.bounds = Some(bounds);
        self
    }

    pub fn studio_lighting(&mut self, studio: StudioLighting) -> &mut Self {
        self.editor.studio = Some(studio);
        self
    }

    pub fn workbench(&mut self, effects: WorkbenchEffects) -> &mut Self {
        self.editor.workbench = effects;
        self
    }

    pub fn editor_grid(&mut self, grid: EditorGrid) -> &mut Self {
        self.editor.grid = Some(grid);
        self
    }

    pub fn look(&mut self, look: Look) -> &mut Self {
        self.look = Some(look);
        self
    }
    pub fn look_mut(&mut self) -> Option<&mut Look> {
        self.look.as_mut()
    }

    pub fn figure(&mut self, figure: Figure) -> FigureId {
        self.figures.push(figure);
        FigureId(self.figures.len() as u32)
    }

    pub fn fluid(&mut self, layer: FluidLayer) -> &mut Self {
        self.fluids.push(layer);
        self
    }

    pub fn wetness(&mut self, wet: &ComputeBuffer, ready: &Ticket) -> Wetness {
        self.wets.push(ComputeSource::new(wet, ready));
        Wetness {
            source: self.wets.len() as u32 - 1,
            first: 0,
        }
    }

    pub fn drape(&mut self, drape: &ComputeBuffer, ready: &Ticket) -> Drape {
        self.drapes.push(ComputeSource::new(drape, ready));
        Drape {
            source: self.drapes.len() as u32 - 1,
            first: 0,
        }
    }

    pub fn time(&mut self, time: f32) -> &mut Self {
        self.time = time;
        self
    }

    pub fn cut(&mut self) -> &mut Self {
        self.cut = true;
        self
    }

    pub fn overlay(&mut self, batch: OverlayBatch) -> &mut Self {
        self.overlays.batches.push(batch);
        self
    }

    pub fn overlays(&mut self, batches: impl IntoIterator<Item = OverlayBatch>) -> &mut Self {
        self.overlays.batches.extend(batches);
        self
    }

    pub fn overlay_data(&mut self, data: &OverlayData<'_>) -> OverlaySource {
        self.overlays.add(data)
    }

    pub fn camera_mut(&mut self) -> &mut Camera {
        &mut self.camera
    }

    pub fn is_empty(&self) -> bool {
        self.draws.is_empty() && self.fluids.is_empty() && self.overlays.is_empty()
    }
}
