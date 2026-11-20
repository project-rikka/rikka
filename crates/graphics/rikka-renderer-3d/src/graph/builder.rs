use std::ops::Range;

use ash::vk;

use crate::error::Result;
use crate::fluid::FluidFrame;
use crate::frame_data::FrameAddresses;
use crate::graph::recorders::{RecorderStore, Recorders};
use crate::graph::resource::{
    Attachment, Buffer, BufferUse, ImageId, ImageSpec, ImageUse, Import, Keep, Size,
};
use crate::graph::store::BoundImage;
use crate::record::Record;
use crate::temporal::Resolution;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Source {
    Created(ImageSpec),
    Imported(Import),
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ImageNode {
    pub source: Source,
    pub published: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct ImageAccessDecl {
    pub image: ImageId,
    pub usage: ImageUse,
    pub stage: vk::PipelineStageFlags2,
    pub discard: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct BufferAccessDecl {
    pub buffer: Buffer,
    pub usage: BufferUse,
}

#[derive(Clone, Default)]
pub(super) struct Rendering {
    pub colors: smallvec::SmallVec<[Attachment; 2]>,
    pub depth: Option<Attachment>,
    pub area: Option<vk::Rect2D>,
}

#[derive(Clone)]
pub(super) enum PassKind {
    Compute,
    Graphics(Rendering),
}

impl PassKind {
    fn shader_stage(&self) -> vk::PipelineStageFlags2 {
        match self {
            Self::Compute => vk::PipelineStageFlags2::COMPUTE_SHADER,
            Self::Graphics(_) => vk::PipelineStageFlags2::FRAGMENT_SHADER,
        }
    }
}

#[derive(Clone)]
pub(super) struct PassNode {
    pub name: &'static str,
    pub group: Option<&'static str>,
    pub kind: PassKind,
    pub images: Range<usize>,
    pub buffers: Range<usize>,
}

#[derive(Default)]
pub(crate) struct GraphScratch {
    images: Vec<ImageNode>,
    passes: Vec<PassNode>,
    image_accesses: Vec<ImageAccessDecl>,
    buffer_accesses: Vec<BufferAccessDecl>,
    recorders: RecorderStore,
}

impl GraphScratch {
    pub(super) fn reclaim(graph: FrameGraph<'_>) -> Self {
        let FrameGraph {
            mut images,
            mut passes,
            mut image_accesses,
            mut buffer_accesses,
            recorders,
            ..
        } = graph;
        images.clear();
        passes.clear();
        image_accesses.clear();
        buffer_accesses.clear();
        Self {
            images,
            passes,
            image_accesses,
            buffer_accesses,
            recorders: recorders.into_store(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Extents {
    pub render: vk::Extent2D,
    pub previous: vk::Extent2D,
    pub capacity: vk::Extent2D,
}

pub(crate) struct FrameGraph<'a> {
    pub(super) extents: Extents,
    pub(super) traced: bool,
    extent: vk::Extent2D,
    resolution: Resolution,
    pub(super) images: Vec<ImageNode>,
    pub(super) passes: Vec<PassNode>,
    pub(super) image_accesses: Vec<ImageAccessDecl>,
    pub(super) buffer_accesses: Vec<BufferAccessDecl>,
    pub(super) recorders: Recorders<'a>,
    group: Option<&'static str>,
}

impl<'a> FrameGraph<'a> {
    pub(crate) fn new(extents: Extents, traced: bool, scratch: GraphScratch) -> Self {
        Self {
            extents,
            traced,
            extent: extents.render,
            resolution: Resolution::Render,
            images: scratch.images,
            passes: scratch.passes,
            image_accesses: scratch.image_accesses,
            buffer_accesses: scratch.buffer_accesses,
            recorders: Recorders::new(scratch.recorders),
            group: None,
        }
    }

    pub(crate) fn extent(&self) -> vk::Extent2D {
        self.extent
    }

    pub(crate) fn capacity(&self) -> vk::Extent2D {
        match self.resolution {
            Resolution::Render => self.extents.capacity,
            Resolution::Display => self.extent,
        }
    }

    pub(super) fn physical_extent(&self, spec: ImageSpec) -> vk::Extent2D {
        spec.size.resolve(self.extents.capacity)
    }

    pub(super) fn logical_extent(&self, spec: ImageSpec) -> vk::Extent2D {
        match spec.keep {
            Keep::Previous(_) => spec.size.resolve(self.extents.previous),
            _ => spec.size.resolve(self.extents.render),
        }
    }

    pub(crate) fn enter_display(&mut self, display: vk::Extent2D) {
        self.extent = display;
        self.resolution = Resolution::Display;
    }

    pub(crate) fn create(&mut self, spec: ImageSpec) -> ImageId {
        let spec = match (self.resolution, spec.size) {
            (Resolution::Render, _) | (_, Size::Fixed(..)) => spec,
            (Resolution::Display, size) => {
                let extent = size.resolve(self.extent);
                ImageSpec {
                    size: Size::Fixed(extent.width, extent.height),
                    ..spec
                }
            }
        };
        self.node(Source::Created(spec))
    }

    pub(crate) fn import(&mut self, import: Import) -> ImageId {
        if let Some(at) = self
            .images
            .iter()
            .position(|node| node.source == Source::Imported(import))
        {
            return ImageId(at as u32);
        }
        self.node(Source::Imported(import))
    }

    pub(crate) fn publish(&mut self, image: ImageId) {
        self.images[image.index()].published = true;
    }

    fn node(&mut self, source: Source) -> ImageId {
        self.images.push(ImageNode {
            source,
            published: false,
        });
        ImageId(self.images.len() as u32 - 1)
    }

    pub(crate) fn grouped<T>(
        &mut self,
        group: &'static str,
        declare: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let outer = self.group.replace(group);
        let declared = declare(self);
        self.group = outer;
        declared
    }

    pub(crate) fn compute<'g>(&'g mut self, name: &'static str) -> PassBuilder<'g, 'a> {
        self.pass(name, PassKind::Compute)
    }

    pub(crate) fn render<'g>(&'g mut self, name: &'static str) -> PassBuilder<'g, 'a> {
        self.pass(name, PassKind::Graphics(Rendering::default()))
    }

    fn pass<'g>(&'g mut self, name: &'static str, kind: PassKind) -> PassBuilder<'g, 'a> {
        let images = self.image_accesses.len();
        let buffers = self.buffer_accesses.len();
        let group = self.group;
        PassBuilder {
            graph: self,
            node: PassNode {
                name,
                group,
                kind,
                images: images..images,
                buffers: buffers..buffers,
            },
        }
    }
}

#[must_use]
pub(crate) struct PassBuilder<'g, 'a> {
    graph: &'g mut FrameGraph<'a>,
    node: PassNode,
}

impl<'a> PassBuilder<'_, 'a> {
    fn access(self, image: ImageId, usage: ImageUse, discard: bool) -> Self {
        let stage = match usage.shader_access() {
            true => self.node.kind.shader_stage(),
            false => vk::PipelineStageFlags2::NONE,
        };
        self.access_in(image, usage, stage, discard)
    }

    fn access_in(
        mut self,
        image: ImageId,
        usage: ImageUse,
        stage: vk::PipelineStageFlags2,
        discard: bool,
    ) -> Self {
        let declared = ImageAccessDecl {
            image,
            usage,
            stage,
            discard,
        };
        let accesses = &mut self.graph.image_accesses;
        let range = self.node.images.clone();
        match accesses[range]
            .iter()
            .position(|known| known.image == image)
        {
            Some(at) => {
                let known = &mut accesses[self.node.images.start + at];
                assert!(
                    known.usage == usage,
                    "pass {} uses an image as both {:?} and {usage:?}",
                    self.node.name,
                    known.usage
                );
                known.stage |= stage;
            }
            None => {
                accesses.push(declared);
                self.node.images.end += 1;
            }
        }
        self
    }

    pub(crate) fn read(self, image: ImageId) -> Self {
        self.access(image, ImageUse::Sampled, false)
    }

    pub(crate) fn read_vertex(self, image: ImageId) -> Self {
        let stage = vk::PipelineStageFlags2::VERTEX_SHADER | self.node.kind.shader_stage();
        self.access_in(image, ImageUse::Sampled, stage, false)
    }

    pub(crate) fn read_all(mut self, images: impl IntoIterator<Item = ImageId>) -> Self {
        for image in images {
            self = self.read(image);
        }
        self
    }

    pub(crate) fn write(self, image: ImageId) -> Self {
        self.access(image, ImageUse::Write, true)
    }

    pub(crate) fn modify(self, image: ImageId) -> Self {
        self.access(image, ImageUse::Modify, false)
    }

    pub(crate) fn color(mut self, attachment: Attachment) -> Self {
        self = self.access(attachment.image, ImageUse::Color, !attachment.load.keeps());
        if let Some((target, _)) = attachment.resolve {
            self = self.access(target, ImageUse::Color, true);
        }
        if let PassKind::Graphics(rendering) = &mut self.node.kind {
            rendering.colors.push(attachment);
        }
        self
    }

    pub(crate) fn depth(mut self, attachment: Attachment) -> Self {
        self = self.access(attachment.image, ImageUse::Depth, !attachment.load.keeps());
        if let Some((target, _)) = attachment.resolve {
            self = self.access(target, ImageUse::DepthResolve, true);
        }
        if let PassKind::Graphics(rendering) = &mut self.node.kind {
            rendering.depth = Some(attachment);
        }
        self
    }

    pub(crate) fn area(mut self, area: vk::Rect2D) -> Self {
        if let PassKind::Graphics(rendering) = &mut self.node.kind {
            rendering.area = Some(area);
        }
        self
    }

    pub(crate) fn buffer(mut self, buffer: Buffer, usage: BufferUse) -> Self {
        self.graph
            .buffer_accesses
            .push(BufferAccessDecl { buffer, usage });
        self.node.buffers.end += 1;
        self
    }

    pub(crate) fn record(
        self,
        record: impl FnOnce(&mut Record<'_>, &mut GpuFrameCtx<'_>, &PassScope<'_>) -> Result<()> + 'a,
    ) {
        let Self { graph, node } = self;
        graph.passes.push(node);
        graph.recorders.push(record);
    }
}

pub(crate) struct PassScope<'s> {
    pub(super) images: &'s [BoundImage],
    pub(super) nodes: &'s [ImageNode],
    pub(super) accesses: &'s [ImageAccessDecl],
    pub addresses: &'s FrameAddresses,
    pub fluid: Option<&'s FluidFrame>,
}

impl PassScope<'_> {
    pub(crate) fn frame(&self) -> u64 {
        self.addresses.refs
    }

    pub(crate) fn sampled(&self, image: ImageId) -> u32 {
        debug_assert!(
            self.nodes[image.index()].published || self.declares(image, ImageUse::Sampled),
            "pass samples {image:?} without declaring the read"
        );
        self.images[image.index()].sampled
    }

    pub(crate) fn storage(&self, image: ImageId) -> u32 {
        debug_assert!(
            self.declares(image, ImageUse::Write) || self.declares(image, ImageUse::Modify),
            "pass stores to {image:?} without declaring the write"
        );
        self.images[image.index()].storage
    }

    fn declares(&self, image: ImageId, usage: ImageUse) -> bool {
        self.accesses
            .iter()
            .any(|access| access.image == image && access.usage == usage)
    }

    pub(crate) fn extent(&self, image: ImageId) -> vk::Extent2D {
        self.images[image.index()].extent
    }

    pub(crate) fn physical_extent(&self, image: ImageId) -> vk::Extent2D {
        self.images[image.index()].physical
    }

    pub(crate) fn bound(&self, image: ImageId) -> BoundImage {
        self.images[image.index()]
    }

    pub(crate) fn fresh(&self, image: ImageId) -> bool {
        self.images[image.index()].fresh
    }

    pub(crate) fn fluid(&self) -> &FluidFrame {
        self.fluid
            .expect("fluid passes run only in frames with fluid")
    }
}
