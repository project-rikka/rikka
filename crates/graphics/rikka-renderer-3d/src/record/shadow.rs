use ash::vk;

use crate::build::{BuiltFrame, ShadowClass};
use crate::error::Result;
use crate::fluid::FluidFrame;
use crate::frame_data::FrameAddresses;
use crate::gpu::GpuPassPush;
use crate::graph::{Attachment, FrameGraph, Load};
use crate::material::Cull;
use crate::pipeline::DrawState;
use crate::recipes;
use crate::record::command::{clear_tile, set_tile};
use crate::record::cull::draws_read;
use crate::record::depth::{CLEAR_DEPTH, clear_depth};
use crate::record::stage::{SceneImages, Stage};
use crate::record::{PASS_SHADOW, Record};
use crate::shadow::{Atlas, ShadowView};
use rikka_gpu::core::frame_ctx::GpuFrameCtx;

const TINT_DIVISOR: u32 = 2;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Layer {
    Depth,
    Figures,
    TintDepth,
    Tint,
}

impl Layer {
    fn draws(self, class: &ShadowClass) -> bool {
        match self {
            Self::Depth | Self::Figures => !class.tint,
            Self::TintDepth | Self::Tint => class.tint,
        }
    }

    fn divisor(self) -> u32 {
        match self {
            Self::Depth | Self::Figures => 1,
            Self::TintDepth | Self::Tint => TINT_DIVISOR,
        }
    }

    fn atlas(self) -> Atlas {
        match self {
            Self::Figures => Atlas::Figures,
            Self::Depth | Self::TintDepth | Self::Tint => Atlas::Main,
        }
    }

    fn tile_clear(self) -> Option<vk::ClearAttachment> {
        match self {
            Self::Figures => None,
            Self::Depth | Self::TintDepth => Some(vk::ClearAttachment {
                aspect_mask: vk::ImageAspectFlags::DEPTH,
                color_attachment: 0,
                clear_value: vk::ClearValue {
                    depth_stencil: vk::ClearDepthStencilValue {
                        depth: CLEAR_DEPTH,
                        stencil: 0,
                    },
                },
            }),
            Self::Tint => Some(vk::ClearAttachment {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                color_attachment: 0,
                clear_value: vk::ClearValue {
                    color: vk::ClearColorValue { float32: [1.0; 4] },
                },
            }),
        }
    }
}

impl Record<'_> {
    pub(super) fn declare_shadow<'f>(
        &mut self,
        graph: &mut FrameGraph<'f>,
        stage: Stage<'f>,
        scene: &SceneImages,
    ) {
        let views = stage.shadows;
        let built = stage.built;
        let fluid = !stage.frame.fluids.is_empty();
        let Some(atlas) = scene.atlas else {
            return;
        };
        if views.is_empty() || (built.instances.is_empty() && !fluid) {
            return;
        }
        let mut pass = graph
            .render("rikka.three.shadow")
            .depth(Attachment::new(atlas, Load::Keep));
        for (buffer, usage) in draws_read() {
            pass = pass.buffer(buffer, usage);
        }
        pass.record(move |record, ctx, scope| {
            record.shadow_pass(
                ctx,
                built,
                scope.addresses,
                views,
                Layer::Depth,
                scope.fluid,
            )
        });
        let figured = views.iter().any(|view| view.atlas == Atlas::Figures);
        if let (Some(figure_atlas), true) = (scene.figure_atlas, figured) {
            let mut pass = graph
                .render("rikka.three.figure_shadow")
                .depth(Attachment::new(figure_atlas, clear_depth()));
            for (buffer, usage) in draws_read() {
                pass = pass.buffer(buffer, usage);
            }
            pass.record(move |record, ctx, scope| {
                record.shadow_pass(ctx, built, scope.addresses, views, Layer::Figures, None)
            });
        }
        let Some(tint) = scene.tint else {
            return;
        };
        let mut depth = graph
            .render("rikka.three.shadow_tint_depth")
            .depth(Attachment::new(tint.depth, Load::Keep));
        for (buffer, usage) in draws_read() {
            depth = depth.buffer(buffer, usage);
        }
        depth.record(move |record, ctx, scope| {
            record.shadow_pass(ctx, built, scope.addresses, views, Layer::TintDepth, None)
        });
        let mut color = graph
            .render("rikka.three.shadow_tint")
            .color(Attachment::new(tint.color, Load::Keep));
        for (buffer, usage) in draws_read() {
            color = color.buffer(buffer, usage);
        }
        color.record(move |record, ctx, scope| {
            record.shadow_pass(ctx, built, scope.addresses, views, Layer::Tint, None)
        });
    }

    fn shadow_pass(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        built: &BuiltFrame,
        addresses: &FrameAddresses,
        views: &[ShadowView],
        layer: Layer,
        fluid: Option<&FluidFrame>,
    ) -> Result<()> {
        let draw = match layer {
            Layer::Tint => DrawState::NO_DEPTH,
            Layer::Depth | Layer::Figures | Layer::TintDepth => {
                DrawState::depth_tested(Cull::None, vk::CompareOp::GREATER, true)
            }
        };
        for (index, view) in views.iter().enumerate() {
            if view.is_cached() || view.atlas != layer.atlas() {
                continue;
            }
            set_tile(ctx, view, layer.divisor());
            if let Some(clear) = layer.tile_clear() {
                clear_tile(ctx, view, layer.divisor(), clear);
            }
            self.push(
                ctx,
                &GpuPassPush {
                    frame: addresses.refs,
                    slot: index as u32,
                    flags: PASS_SHADOW,
                },
            );
            for (number, class) in built.shadow_classes.iter().enumerate() {
                if !layer.draws(class) {
                    continue;
                }
                let bucket = built.layout.shadow(index as u32, number as u32);
                let program = class
                    .program
                    .map(|program| &*built.shadow_programs[program as usize]);
                let recipe = match (layer, program) {
                    (Layer::Tint, Some(program)) => {
                        recipes::shadow_tint(&self.mesh.builtin, program)
                    }
                    _ => recipes::shadow(&self.mesh.builtin, program, class.masked),
                };
                let pipeline = self.pipeline(&recipe)?;
                self.bind(ctx, pipeline, draw);
                self.draw_range(
                    ctx,
                    addresses,
                    bucket,
                    built.bucket_table[bucket as usize],
                    class.indices,
                );
            }
            if let Some(fluid) = fluid {
                self.fluid_shadow(ctx, fluid, addresses, index as u32)?;
            }
        }
        Ok(())
    }
}
