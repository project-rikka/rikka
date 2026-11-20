use std::any::Any;

use rikka_gpu::core::convert::YcbcrConverter;
use rikka_gpu::core::runtime::GpuRuntime;
use rikka_gpu::reclaim::Reclaimer;
use rikka_gpu::texture::TexturePool;

use crate::renderer::Renderer;

pub trait Extension: Any + Send {
    fn has_pending(&self) -> bool;

    fn reclaim(
        &mut self,
        request: Box<dyn Any + Send>,
        gpu: &mut GpuRuntime,
        textures: &mut TexturePool,
    );
}

#[derive(Default)]
pub struct ExtensionSlot {
    held: Option<Box<dyn Extension>>,
}

impl ExtensionSlot {
    pub fn get<T: Extension>(&mut self) -> Option<&mut T> {
        let held: &mut dyn Any = self.held.as_deref_mut()?;
        held.downcast_mut()
    }

    pub fn insert<T: Extension>(&mut self, extension: T) -> &mut T {
        self.held = Some(Box::new(extension));
        self.get().expect("the slot holds what was just put in")
    }

    pub(crate) fn has_pending(&self) -> bool {
        self.held
            .as_deref()
            .is_some_and(|extension| extension.has_pending())
    }

    pub(crate) fn reclaim(
        &mut self,
        request: Box<dyn Any + Send>,
        gpu: &mut GpuRuntime,
        textures: &mut TexturePool,
    ) {
        if let Some(extension) = self.held.as_deref_mut() {
            extension.reclaim(request, gpu, textures);
        }
    }
}

impl std::fmt::Debug for ExtensionSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExtensionSlot")
            .field("held", &self.held.is_some())
            .finish()
    }
}

pub struct RendererParts<'a> {
    pub gpu: &'a mut GpuRuntime,
    pub textures: &'a mut TexturePool,
    pub convert: &'a mut YcbcrConverter,
    pub reclaimer: &'a Reclaimer,
    pub extension: &'a mut ExtensionSlot,
}

impl std::fmt::Debug for RendererParts<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RendererParts").finish_non_exhaustive()
    }
}

impl Renderer {
    pub fn parts(&mut self) -> RendererParts<'_> {
        RendererParts {
            gpu: &mut self.gpu,
            textures: &mut self.sources.textures,
            convert: &mut self.sources.convert,
            reclaimer: &self.reclaimer,
            extension: &mut self.extension,
        }
    }
}
