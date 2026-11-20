use std::any::Any;

use rikka_gpu::core::runtime::GpuRuntime;
use rikka_gpu::error::Capability;
use rikka_gpu::texture::TexturePool;
use rikka_ies::IesProfile;
use rikka_renderer::{Extension, ExtensionSlot, Renderer, RendererParts};
use rikka_slang::ParameterSchema;

use crate::draw::Frame;
use crate::effect::{Effect, EffectId, EffectReadiness};
use crate::error::{Error, Resource, Result};
use crate::gpu::NONE_INDEX;
use crate::light::LightProfileId;
use crate::material::{MaterialDesc, MaterialId, TextureBinding, Value};
use crate::mesh::{Indices, MeshData, MeshId, MeshUpdate, Submesh};
use crate::overlay::{OverlayBufferId, OverlayData, OverlayUpdate};
use crate::pick::{self, Pick, PickArea};
use crate::reclaim::Reclaimed;
use crate::record::Record;
use crate::state::Renderer3dState;
use crate::viewport::{Viewport, ViewportConfig};

pub trait Three {
    fn three(&mut self) -> Renderer3d<'_>;
}

impl Three for Renderer {
    fn three(&mut self) -> Renderer3d<'_> {
        Renderer3d { renderer: self }
    }
}

#[derive(Debug)]
pub struct Renderer3d<'a> {
    renderer: &'a mut Renderer,
}

impl Extension for Renderer3dState {
    fn has_pending(&self) -> bool {
        Renderer3dState::has_pending(self)
    }

    fn reclaim(
        &mut self,
        request: Box<dyn Any + Send>,
        gpu: &mut GpuRuntime,
        textures: &mut TexturePool,
    ) {
        let Ok(request) = request.downcast::<Reclaimed>() else {
            return;
        };
        match *request {
            Reclaimed::Mesh(id) => {
                self.destroy_mesh(id);
            }
            Reclaimed::OverlayBuffer(id) => {
                self.overlays.destroy(id);
            }
            Reclaimed::Material(id) => {
                let after = gpu.next_value();
                self.destroy_material(id, after);
            }
            Reclaimed::Viewport(parts) => parts.release(&mut gpu.recycler(), textures),
        }
    }
}

fn ensured<'a>(
    extension: &'a mut ExtensionSlot,
    gpu: &GpuRuntime,
) -> Result<&'a mut Renderer3dState> {
    if extension.get::<Renderer3dState>().is_none() {
        if !gpu.device().three_d() {
            return Err(no_mesh());
        }
        extension.insert(Renderer3dState::new(
            gpu.device().clone(),
            gpu.table_layout(),
        )?);
    }
    Ok(extension.get().expect("the 3D state was just ensured"))
}

fn built<'a>(
    extension: &'a mut ExtensionSlot,
    gpu: &GpuRuntime,
) -> Result<Option<&'a mut Renderer3dState>> {
    match gpu.device().three_d() {
        true => Ok(extension.get()),
        false => Err(no_mesh()),
    }
}

fn grow_textures(
    gpu: &mut GpuRuntime,
    textures: &mut TexturePool,
    bindings: impl Iterator<Item = TextureBinding>,
) {
    let retire_at = gpu.next_value();
    let mut bind = gpu.bind_point();
    for binding in bindings {
        textures.grow(&mut bind, retire_at, binding.texture);
    }
}

impl Renderer3d<'_> {
    pub fn create_mesh(&mut self, data: &MeshData<'_>) -> Result<MeshId> {
        let RendererParts { gpu, extension, .. } = self.renderer.parts();
        ensured(extension, gpu)?.create_mesh(data)
    }

    pub fn update_mesh(&mut self, id: MeshId, update: &MeshUpdate<'_>) -> Result<()> {
        let RendererParts { gpu, extension, .. } = self.renderer.parts();
        match built(extension, gpu)? {
            Some(state) => state.update_mesh(id, update),
            None => Err(Error::Unknown(Resource::Mesh(id))),
        }
    }

    pub fn replace_mesh(&mut self, id: MeshId, data: &MeshData<'_>) -> Result<()> {
        let RendererParts { gpu, extension, .. } = self.renderer.parts();
        match built(extension, gpu)? {
            Some(state) => state.replace_mesh(id, data),
            None => Err(Error::Unknown(Resource::Mesh(id))),
        }
    }

    pub fn replace_mesh_indices(
        &mut self,
        id: MeshId,
        indices: Indices<'_>,
        submeshes: &[Submesh],
    ) -> Result<()> {
        let RendererParts { gpu, extension, .. } = self.renderer.parts();
        match built(extension, gpu)? {
            Some(state) => state.replace_mesh_indices(id, indices, submeshes),
            None => Err(Error::Unknown(Resource::Mesh(id))),
        }
    }

    pub fn set_mesh_edges(&mut self, id: MeshId, submesh: u32, edges: &[[u32; 4]]) -> Result<()> {
        let RendererParts { gpu, extension, .. } = self.renderer.parts();
        match built(extension, gpu)? {
            Some(state) => state.set_mesh_edges(id, submesh, edges),
            None => Err(Error::Unknown(Resource::Mesh(id))),
        }
    }

    pub fn destroy_mesh(&mut self, id: MeshId) -> Result<bool> {
        let RendererParts { gpu, extension, .. } = self.renderer.parts();
        Ok(built(extension, gpu)?.is_some_and(|mesh| mesh.destroy_mesh(id)))
    }

    pub fn create_overlay_buffer(&mut self, data: &OverlayData<'_>) -> Result<OverlayBufferId> {
        let RendererParts { gpu, extension, .. } = self.renderer.parts();
        ensured(extension, gpu)?.overlays.create(data)
    }

    pub fn update_overlay_buffer(
        &mut self,
        id: OverlayBufferId,
        update: &OverlayUpdate<'_>,
    ) -> Result<()> {
        let RendererParts { gpu, extension, .. } = self.renderer.parts();
        match built(extension, gpu)? {
            Some(state) => state.overlays.update(id, update),
            None => Err(Error::Unknown(Resource::OverlayBuffer(id))),
        }
    }

    pub fn update_overlay_indices(
        &mut self,
        id: OverlayBufferId,
        first: u32,
        indices: &[u32],
    ) -> Result<()> {
        let RendererParts { gpu, extension, .. } = self.renderer.parts();
        match built(extension, gpu)? {
            Some(state) => state.overlays.update_indices(id, first, indices),
            None => Err(Error::Unknown(Resource::OverlayBuffer(id))),
        }
    }

    pub fn destroy_overlay_buffer(&mut self, id: OverlayBufferId) -> Result<bool> {
        let RendererParts { gpu, extension, .. } = self.renderer.parts();
        Ok(built(extension, gpu)?.is_some_and(|state| state.overlays.destroy(id)))
    }

    pub fn create_material(&mut self, desc: MaterialDesc) -> Result<MaterialId> {
        let RendererParts {
            gpu,
            textures,
            extension,
            ..
        } = self.renderer.parts();
        let finished = gpu.completed_value();
        let mesh = ensured(extension, gpu)?;
        grow_textures(gpu, textures, desc.values.iter().filter_map(Value::texture));
        mesh.create_material(desc, finished, |id| textures.slot(id))
    }

    pub fn destroy_material(&mut self, id: MaterialId) -> Result<bool> {
        let RendererParts { gpu, extension, .. } = self.renderer.parts();
        let after = gpu.next_value();
        Ok(built(extension, gpu)?.is_some_and(|mesh| mesh.destroy_material(id, after)))
    }

    pub fn set_material(
        &mut self,
        id: MaterialId,
        name: &str,
        value: impl Into<Value>,
    ) -> Result<bool> {
        self.set_material_values(id, &[(name, value.into())])
    }

    pub fn set_material_values(
        &mut self,
        id: MaterialId,
        values: &[(&str, Value)],
    ) -> Result<bool> {
        let RendererParts {
            gpu,
            textures,
            extension,
            ..
        } = self.renderer.parts();
        let mesh = built(extension, gpu)?;
        grow_textures(
            gpu,
            textures,
            values.iter().filter_map(|(_, value)| value.texture()),
        );
        let after = gpu.next_value();
        match mesh {
            Some(mesh) => mesh.set_material_values(id, values, after, |id| textures.slot(id)),
            None => Err(Error::Unknown(Resource::Material(id))),
        }
    }

    /// Uploads an LM-63 profile for [`Light::profile`](crate::Light::profile).
    pub fn create_light_profile(&mut self, profile: &IesProfile) -> Result<LightProfileId> {
        let RendererParts { gpu, extension, .. } = self.renderer.parts();
        let finished = gpu.completed_value();
        ensured(extension, gpu)?.profiles.create(profile, finished)
    }

    /// Frees a profile once frames that may use it finish; lights still naming it fail to draw.
    pub fn destroy_light_profile(&mut self, id: LightProfileId) -> Result<bool> {
        let RendererParts { gpu, extension, .. } = self.renderer.parts();
        let after = gpu.next_value();
        Ok(built(extension, gpu)?.is_some_and(|mesh| mesh.profiles.destroy(id, after)))
    }

    pub fn create_effect(&mut self, effect: &Effect) -> Result<EffectId> {
        let RendererParts { gpu, extension, .. } = self.renderer.parts();
        ensured(extension, gpu)?.create_effect(effect)
    }

    pub fn reload_effect(&mut self, id: EffectId, effect: &Effect) -> Result<()> {
        let RendererParts { gpu, extension, .. } = self.renderer.parts();
        ensured(extension, gpu)?.reload_effect(id, effect)
    }

    pub fn destroy_effect(&mut self, id: EffectId) -> Result<bool> {
        let RendererParts { gpu, extension, .. } = self.renderer.parts();
        Ok(built(extension, gpu)?.is_some_and(|mesh| mesh.effects.destroy(id)))
    }

    pub fn effect_schema(&mut self, id: EffectId) -> Result<ParameterSchema> {
        let RendererParts { gpu, extension, .. } = self.renderer.parts();
        ensured(extension, gpu)?.effects.schema(id)
    }

    pub fn effect_readiness(&mut self, id: EffectId) -> Result<EffectReadiness> {
        let RendererParts { gpu, extension, .. } = self.renderer.parts();
        ensured(extension, gpu)?.effects.readiness(id)
    }

    pub fn create_viewport(&mut self, config: ViewportConfig) -> Result<Viewport> {
        let RendererParts {
            gpu,
            textures,
            reclaimer,
            extension,
            ..
        } = self.renderer.parts();
        let mesh = ensured(extension, gpu)?;
        let mut bind = gpu.bind_point();
        mesh.create_viewport(&mut bind, textures, config, reclaimer.clone())
    }

    pub fn render(&mut self, viewport: &mut Viewport, frame: &Frame) -> Result<()> {
        self.renderer.reclaim();
        let RendererParts {
            gpu,
            textures,
            convert,
            extension,
            ..
        } = self.renderer.parts();
        let mesh = ensured(extension, gpu)?;
        grow_textures(
            gpu,
            textures,
            frame
                .look
                .iter()
                .flat_map(|look| look.overrides.iter())
                .chain(
                    frame
                        .figures
                        .iter()
                        .flat_map(|figure| figure.overrides.iter()),
                )
                .filter_map(|entry| entry.value.texture()),
        );
        let after = gpu.next_value();
        mesh.prepare_materials(frame, after, |id| textures.slot(id))?;
        mesh.effects.adopt_ready();
        let mut ctx = gpu.begin_frame()?;
        mesh.precompile_for(frame);
        let recorded = Record::new(mesh, textures, convert).frame(&mut ctx, viewport, frame);
        let submitted = gpu.submit(&[], &[]);
        let rays = mesh.rays.as_mut();
        match submitted {
            Ok(value) => {
                for former in textures.commit() {
                    gpu.defer_until(value, Box::new(former));
                }
                for former in rays.into_iter().flat_map(|rays| rays.commit()) {
                    gpu.defer_until(value, Box::new(former));
                }
            }
            Err(_) => {
                textures.revert();
                if let Some(rays) = rays {
                    rays.revert();
                }
            }
        }
        recorded?;
        submitted?;
        Ok(())
    }

    pub fn create_pick(&mut self) -> Pick {
        Pick::new(self.renderer.reclaimer())
    }

    pub fn start_pick(
        &mut self,
        viewport: &Viewport,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        into: &mut Pick,
    ) -> Result<()> {
        let identity = viewport.targets().identity_slot;
        if identity == NONE_INDEX {
            return Err(Error::Invalid(
                "viewport has no identity buffer: enable ViewportConfig::identity".into(),
            ));
        }
        if !viewport.drawn {
            return Err(Error::Invalid("viewport has not been drawn yet".into()));
        }
        let RendererParts { gpu, extension, .. } = self.renderer.parts();
        let mesh = extension
            .get::<Renderer3dState>()
            .expect("drawn viewport has a 3D renderer");
        let render = viewport.extent();
        let display = viewport.display_extent();
        let across = |at: i32, length: u32, render: u32, display: u32| {
            let scale = render as f32 / display as f32;
            (
                (at as f32 * scale).floor() as i32,
                ((length as f32 * scale).ceil() as u32).max(1),
            )
        };
        let (x, width) = across(x, width, render.width, display.width);
        let (y, height) = across(y, height, render.height, display.height);
        let area = PickArea::new(x, y, width, height, render);
        pick::start(gpu, mesh, identity, area, into)
    }

    pub fn poll_pick(&mut self, from: &mut Pick) -> bool {
        from.claim(self.renderer.parts().gpu.completed_value())
    }

    pub fn pick(
        &mut self,
        viewport: &Viewport,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Result<Pick> {
        let mut pick = self.create_pick();
        self.start_pick(viewport, x, y, width, height, &mut pick)?;
        if let Some(ready_at) = pick.ready_at() {
            self.renderer.parts().gpu.wait_value(ready_at)?;
        }
        self.poll_pick(&mut pick);
        Ok(pick)
    }
}

fn no_mesh() -> Error {
    Error::not_enabled(Capability::ThreeD)
}
