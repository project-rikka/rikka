use std::collections::HashMap;
use std::os::fd::OwnedFd;

use rikka_compositor_kms::{FramebufferId, Output, Rect};
use rikka_compositor_types::{Format, FormatSet, Modifier};
use rikka_renderer::{
    DamageSource, DmabufFrame, DmabufOutput, DmabufTarget, Node, NodeId, Renderer, Scene, TextureId,
};
use rikka_types::{Color, Rect as SceneRect};

use super::{Depth, LinkError, dmabuf_of};
use crate::render::{renderer_format_to_kms, with_kms_spec};

pub(in crate::backend::kms) struct Copied {
    scene: Scene,
    shown: Option<NodeId>,
    output: DmabufOutput,
    size: (u32, u32),
    pairs: HashMap<FramebufferId, Pair>,
}

struct Pair {
    stage: DmabufTarget,
    texture: TextureId,
    target: DmabufTarget,
}

impl Copied {
    pub(super) fn fourcc(
        copier: &Renderer,
        exported: &FormatSet,
        depth: Depth,
    ) -> Option<rikka_compositor_types::Fourcc> {
        let sampled: FormatSet = copier
            .gpu()
            .dmabuf_texture_formats()
            .iter()
            .map(renderer_format_to_kms)
            .collect();
        depth.scanned().iter().copied().find(|fourcc| {
            let linear = Format::new(*fourcc, Modifier::LINEAR);
            exported.contains(linear) && sampled.contains(linear)
        })
    }

    pub(super) fn new(
        output: &Output,
        copier: &mut Renderer,
        renderer: &mut Renderer,
        fourcc: rikka_compositor_types::Fourcc,
    ) -> Result<Self, LinkError> {
        let mode = output.mode();
        let size = (mode.width(), mode.height());
        let mut scene = Scene::new();
        scene.set_background(Color::BLACK);
        let mut copied = Self {
            scene,
            shown: None,
            output: copier.create_dmabuf_output(DamageSource::CompleteForBuffer),
            size,
            pairs: HashMap::new(),
        };
        for framebuffer in output.swapchain().framebuffers() {
            let paired = output
                .swapchain()
                .dmabuf(framebuffer.id())
                .ok_or(LinkError::NoFormat)
                .and_then(|dmabuf| {
                    let target = with_kms_spec(dmabuf, |spec| copier.import_dmabuf(spec))?;
                    let stage = renderer.export_shared_dmabuf(
                        size.0,
                        size.1,
                        fourcc.0,
                        &[Modifier::LINEAR.0],
                    )?;
                    let shared = dmabuf_of(&stage, size.0, size.1)?;
                    let texture =
                        with_kms_spec(&shared, |spec| copier.textures().import_dmabuf(spec))?;
                    Ok(Pair {
                        stage: stage.target,
                        texture,
                        target,
                    })
                });
            match paired {
                Ok(pair) => {
                    copied.pairs.insert(framebuffer.id(), pair);
                }
                Err(error) => {
                    copied.release(copier);
                    return Err(error);
                }
            }
        }
        Ok(copied)
    }

    pub(in crate::backend::kms) fn stage(
        &mut self,
        framebuffer: FramebufferId,
    ) -> Option<&mut DmabufTarget> {
        self.pairs.get_mut(&framebuffer).map(|pair| &mut pair.stage)
    }

    pub(in crate::backend::kms) fn copy(
        &mut self,
        copier: &mut Renderer,
        framebuffer: FramebufferId,
        damage: &[Rect],
        drawn: OwnedFd,
    ) -> Result<Option<DmabufFrame>, LinkError> {
        let Some(pair) = self.pairs.get_mut(&framebuffer) else {
            return Ok(None);
        };
        let whole = SceneRect::new(0.0, 0.0, self.size.0 as f32, self.size.1 as f32);
        match self.shown {
            Some(node) => {
                self.scene.edit(node).repaint_image(
                    pair.texture,
                    damage.iter().map(|rect| {
                        SceneRect::new(
                            rect.x as f32,
                            rect.y as f32,
                            rect.width as f32,
                            rect.height as f32,
                        )
                    }),
                );
            }
            None => {
                self.shown = Some(
                    self.scene
                        .insert(Node::image(whole, pair.texture).opaque_within(whole)),
                );
            }
        }
        let frame = copier.draw(
            &mut self.scene,
            self.output
                .with_target(&mut pair.target)
                .with_release_fence(drawn),
        )?;
        Ok(Some(frame))
    }

    pub(in crate::backend::kms) fn release(self, copier: &mut Renderer) {
        let textures = copier.textures();
        for pair in self.pairs.values() {
            textures.destroy(pair.texture);
        }
    }
}
