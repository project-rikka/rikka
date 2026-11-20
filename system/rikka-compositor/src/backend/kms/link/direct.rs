use std::collections::HashMap;
use std::collections::hash_map::Entry;

use rikka_compositor_kms::{FramebufferId, Output};
use rikka_renderer::{DmabufTarget, Renderer};

use super::LinkError;
use crate::render::with_kms_spec;

#[derive(Default)]
pub(in crate::backend::kms) struct Direct {
    targets: HashMap<FramebufferId, DmabufTarget>,
}

impl Direct {
    pub(super) fn imported(output: &Output, renderer: &mut Renderer) -> Result<Self, LinkError> {
        let mut direct = Self::default();
        for framebuffer in output.swapchain().framebuffers() {
            let Some(dmabuf) = output.swapchain().dmabuf(framebuffer.id()) else {
                continue;
            };
            let target = with_kms_spec(dmabuf, |spec| renderer.import_dmabuf(spec))?;
            direct.targets.insert(framebuffer.id(), target);
        }
        Ok(direct)
    }

    pub(super) fn exported(output: &Output, targets: Vec<DmabufTarget>) -> Self {
        Self {
            targets: output
                .swapchain()
                .framebuffers()
                .map(|framebuffer| framebuffer.id())
                .zip(targets)
                .collect(),
        }
    }

    pub(in crate::backend::kms) fn target(
        &mut self,
        framebuffer: FramebufferId,
        output: &Output,
        renderer: &mut Renderer,
    ) -> Result<&mut DmabufTarget, LinkError> {
        match self.targets.entry(framebuffer) {
            Entry::Occupied(occupied) => Ok(occupied.into_mut()),
            Entry::Vacant(empty) => {
                let dmabuf = output.swapchain().dmabuf(framebuffer).ok_or_else(|| {
                    LinkError::Display(rikka_compositor_kms::Error::UnknownOutput(
                        output.name().to_owned(),
                    ))
                })?;
                let target = with_kms_spec(dmabuf, |spec| renderer.import_dmabuf(spec))?;
                Ok(empty.insert(target))
            }
        }
    }
}
