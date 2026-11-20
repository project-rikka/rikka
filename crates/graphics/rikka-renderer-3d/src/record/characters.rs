use crate::build::{BuiltFrame, DirectDraw};
use crate::graph::{Attachment, FrameGraph, ImageId, ImageSpec, Size};
use crate::record::Record;
use crate::record::cull::draws_read;
use crate::record::depth::clear_depth;
use crate::record::direct::DirectPass;
use crate::record::stage::{SceneImages, Stage};
use crate::viewport::DEPTH_FORMAT;

#[derive(Clone, Copy)]
enum Layer {
    Hair,
    Face,
}

impl Layer {
    fn name(self) -> &'static str {
        match self {
            Self::Hair => "rikka.three.hair_depth",
            Self::Face => "rikka.three.face_depth",
        }
    }

    fn draws(self, built: &BuiltFrame) -> &[DirectDraw] {
        match self {
            Self::Hair => &built.hair,
            Self::Face => &built.face,
        }
    }
}

impl Record<'_> {
    pub(super) fn declare_characters<'f>(
        &mut self,
        graph: &mut FrameGraph<'f>,
        stage: Stage<'f>,
        scene: &mut SceneImages,
    ) {
        let built = stage.built;
        if stage.look.is_none() || built.hair.is_empty() || built.face.is_empty() {
            return;
        }
        scene.hair = Some(self.declare_layer_depth(graph, built, Layer::Hair));
        scene.face = Some(self.declare_layer_depth(graph, built, Layer::Face));
    }

    fn declare_layer_depth<'f>(
        &mut self,
        graph: &mut FrameGraph<'f>,
        built: &'f BuiltFrame,
        layer: Layer,
    ) -> ImageId {
        let depth =
            graph.create(ImageSpec::new(layer.name(), DEPTH_FORMAT, Size::Viewport).nearest());
        graph.publish(depth);
        let mut pass = graph
            .render(layer.name())
            .depth(Attachment::new(depth, clear_depth()));
        for (buffer, usage) in draws_read() {
            pass = pass.buffer(buffer, usage);
        }
        pass.record(move |record, ctx, scope| {
            record.draw_direct(
                ctx,
                built,
                scope.addresses,
                layer.draws(built),
                DirectPass::CharacterDepth,
            )
        });
        depth
    }
}
