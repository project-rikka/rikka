use crate::error::Result;
use crate::graph::{Attachment, FrameGraph, ImageId, ImageSpec, Load, PassScope, Size};
use crate::record::Record;
use crate::viewport::COLOR_FORMAT;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;

pub(crate) const HDR: &str = "rikka.three.hdr";

#[derive(Clone, Copy, Debug)]
pub(crate) struct Chain {
    current: ImageId,
    format: ash::vk::Format,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Link {
    pub input: ImageId,
    pub output: ImageId,
}

impl Chain {
    pub(crate) fn start(current: ImageId) -> Self {
        Self {
            current,
            format: COLOR_FORMAT,
        }
    }

    pub(crate) fn current(&self) -> ImageId {
        self.current
    }

    pub(crate) fn advance(&mut self, graph: &mut FrameGraph<'_>) -> Link {
        let output = graph.create(ImageSpec::new(HDR, self.format, Size::Viewport));
        self.link_to(output)
    }

    pub(crate) fn link_to(&mut self, output: ImageId) -> Link {
        let link = Link {
            input: self.current,
            output,
        };
        self.current = output;
        link
    }

    pub(crate) fn restart(&mut self, current: ImageId) {
        self.current = current;
    }

    pub(crate) fn filter<'f>(
        &mut self,
        graph: &mut FrameGraph<'f>,
        name: &'static str,
        reads: impl IntoIterator<Item = ImageId>,
        draw: impl FnOnce(&mut Record<'_>, &mut GpuFrameCtx<'_>, &PassScope<'_>, u32) -> Result<()> + 'f,
    ) {
        self.filter_drawing(graph, name, reads, None, draw);
    }

    pub(crate) fn filter_drawing<'f>(
        &mut self,
        graph: &mut FrameGraph<'f>,
        name: &'static str,
        reads: impl IntoIterator<Item = ImageId>,
        vertex_reads: impl IntoIterator<Item = ImageId>,
        draw: impl FnOnce(&mut Record<'_>, &mut GpuFrameCtx<'_>, &PassScope<'_>, u32) -> Result<()> + 'f,
    ) {
        let link = self.advance(graph);
        let mut pass = graph
            .render(name)
            .color(Attachment::new(link.output, Load::Discard))
            .read(link.input)
            .read_all(reads);
        for image in vertex_reads {
            pass = pass.read_vertex(image);
        }
        pass.record(move |record, ctx, scope| draw(record, ctx, scope, scope.sampled(link.input)));
    }

    pub(crate) fn filter_compute<'f>(
        &mut self,
        graph: &mut FrameGraph<'f>,
        name: &'static str,
        reads: impl IntoIterator<Item = ImageId>,
        dispatch: impl FnOnce(
            &mut Record<'_>,
            &mut GpuFrameCtx<'_>,
            &PassScope<'_>,
            Written,
        ) -> Result<()>
        + 'f,
    ) {
        let link = self.advance(graph);
        graph
            .compute(name)
            .read(link.input)
            .read_all(reads)
            .write(link.output)
            .record(move |record, ctx, scope| {
                let written = Written {
                    input: scope.sampled(link.input),
                    storage: scope.storage(link.output),
                    extent: scope.extent(link.output),
                };
                dispatch(record, ctx, scope, written)
            });
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Written {
    pub input: u32,
    pub storage: u32,
    pub extent: ash::vk::Extent2D,
}
