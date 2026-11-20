use ash::vk;

use rikka_gpu::core::image::ImageAccess;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ImageId(pub(super) u32);

impl ImageId {
    pub(super) fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Filter {
    Linear,
    Nearest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Size {
    Viewport,
    Divided(u32),
    Fixed(u32, u32),
}

impl Size {
    pub(crate) fn resolve(self, viewport: vk::Extent2D) -> vk::Extent2D {
        let extent = match self {
            Self::Viewport => viewport,
            Self::Divided(divisor) => scaled(viewport, divisor),
            Self::Fixed(width, height) => vk::Extent2D { width, height },
        };
        vk::Extent2D {
            width: extent.width.max(1),
            height: extent.height.max(1),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Keep {
    Transient,
    History(u64),
    Current(u64),
    Previous(u64),
    Shared,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ImageSpec {
    pub name: &'static str,
    pub format: vk::Format,
    pub size: Size,
    pub filter: Filter,
    pub keep: Keep,
    pub samples: vk::SampleCountFlags,
}

impl ImageSpec {
    pub(crate) fn new(name: &'static str, format: vk::Format, size: Size) -> Self {
        Self {
            name,
            format,
            size,
            filter: Filter::Linear,
            keep: Keep::Transient,
            samples: vk::SampleCountFlags::TYPE_1,
        }
    }

    pub(crate) fn nearest(mut self) -> Self {
        self.filter = Filter::Nearest;
        self
    }

    pub(crate) fn history(mut self, key: u64) -> Self {
        self.keep = Keep::History(key);
        self
    }

    pub(crate) fn current(mut self, key: u64) -> Self {
        self.keep = Keep::Current(key);
        self
    }

    pub(crate) fn previous(mut self, key: u64) -> Self {
        self.keep = Keep::Previous(key);
        self
    }

    pub(crate) fn shared(mut self) -> Self {
        self.keep = Keep::Shared;
        self
    }

    pub(crate) fn multisampled(mut self, samples: vk::SampleCountFlags) -> Self {
        self.samples = samples;
        self
    }

    pub(super) fn sampleable(&self) -> bool {
        self.samples == vk::SampleCountFlags::TYPE_1
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Import {
    ColorTarget,
    Resolved,
    DepthTarget,
    DepthResolved,
    IdentityTarget,
    Identity,
    Outline,
    Hzb,
    Output,
    ShadowAtlas,
    ShadowTint,
    ShadowTintDepth,
    FigureAtlas,
}

impl Import {
    pub(crate) const COUNT: usize = 13;

    pub(crate) fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ImageUse {
    Sampled,
    Write,
    Modify,
    Color,
    Depth,
    DepthResolve,
}

impl ImageUse {
    pub(super) fn writes(self) -> bool {
        !matches!(self, Self::Sampled)
    }

    pub(super) fn usage(self, format: vk::Format) -> vk::ImageUsageFlags {
        match self {
            Self::Sampled => vk::ImageUsageFlags::SAMPLED,
            Self::Write | Self::Modify => vk::ImageUsageFlags::STORAGE,
            Self::Color => vk::ImageUsageFlags::COLOR_ATTACHMENT,
            Self::Depth | Self::DepthResolve => match is_depth(format) {
                true => vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
                false => vk::ImageUsageFlags::COLOR_ATTACHMENT,
            },
        }
    }

    pub(super) fn access(
        self,
        stage: vk::PipelineStageFlags2,
        sampled_layout: vk::ImageLayout,
    ) -> ImageAccess {
        match self {
            Self::Sampled => ImageAccess {
                stage,
                access: vk::AccessFlags2::SHADER_SAMPLED_READ,
                layout: sampled_layout,
            },
            Self::Write | Self::Modify => ImageAccess {
                stage,
                ..ImageAccess::COMPUTE_STORAGE
            },
            Self::Color => ImageAccess::COLOR_ATTACHMENT,
            Self::Depth => DEPTH_TARGET,
            Self::DepthResolve => DEPTH_RESOLVE_TARGET,
        }
    }

    pub(super) fn shader_access(self) -> bool {
        matches!(self, Self::Sampled | Self::Write | Self::Modify)
    }
}

pub(crate) const DEPTH_TARGET: ImageAccess = ImageAccess {
    stage: vk::PipelineStageFlags2::from_raw(
        vk::PipelineStageFlags2::EARLY_FRAGMENT_TESTS.as_raw()
            | vk::PipelineStageFlags2::LATE_FRAGMENT_TESTS.as_raw()
            | vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT.as_raw(),
    ),
    access: vk::AccessFlags2::from_raw(
        vk::AccessFlags2::DEPTH_STENCIL_ATTACHMENT_READ.as_raw()
            | vk::AccessFlags2::DEPTH_STENCIL_ATTACHMENT_WRITE.as_raw()
            | vk::AccessFlags2::COLOR_ATTACHMENT_READ.as_raw(),
    ),
    layout: vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL,
};

const DEPTH_RESOLVE_TARGET: ImageAccess = ImageAccess {
    stage: vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
    access: vk::AccessFlags2::COLOR_ATTACHMENT_WRITE,
    layout: vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL,
};

#[derive(Clone, Copy)]
pub(crate) enum Load {
    Clear(vk::ClearValue),
    Keep,
    Discard,
}

impl Load {
    pub(super) fn op(self) -> vk::AttachmentLoadOp {
        match self {
            Self::Clear(_) => vk::AttachmentLoadOp::CLEAR,
            Self::Keep => vk::AttachmentLoadOp::LOAD,
            Self::Discard => vk::AttachmentLoadOp::DONT_CARE,
        }
    }

    pub(super) fn keeps(self) -> bool {
        matches!(self, Self::Keep)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Attachment {
    pub image: ImageId,
    pub load: Load,
    pub store: bool,
    pub resolve: Option<(ImageId, vk::ResolveModeFlags)>,
}

impl Attachment {
    pub(crate) fn new(image: ImageId, load: Load) -> Self {
        Self {
            image,
            load,
            store: true,
            resolve: None,
        }
    }

    pub(crate) fn clear_color(image: ImageId, color: vk::ClearColorValue) -> Self {
        Self::new(image, Load::Clear(vk::ClearValue { color }))
    }

    pub(crate) fn transient(mut self) -> Self {
        self.store = false;
        self
    }

    pub(crate) fn resolve(mut self, target: ImageId, mode: vk::ResolveModeFlags) -> Self {
        self.resolve = Some((target, mode));
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Buffer {
    Deform,
    Draws,
    Clusters,
    Environment,
    History,
    Scene,
    Particles,
}

impl Buffer {
    pub(super) const COUNT: usize = 7;

    pub(super) fn index(self) -> usize {
        self as usize
    }

    pub(super) fn carried(self, traced: bool) -> Carried {
        match self {
            Self::Deform => {
                let built = match traced {
                    true => vk::PipelineStageFlags2::ACCELERATION_STRUCTURE_BUILD_KHR,
                    false => vk::PipelineStageFlags2::NONE,
                };
                Carried::read(SHADER_STAGES | vk::PipelineStageFlags2::TRANSFER | built)
            }
            Self::Environment => Carried::read(SHADER_STAGES),
            Self::History => Carried {
                write: Some(BufferUse::new(
                    vk::PipelineStageFlags2::TRANSFER,
                    vk::AccessFlags2::TRANSFER_WRITE,
                )),
                readers: vk::PipelineStageFlags2::NONE,
            },
            Self::Scene => Carried::read(
                vk::PipelineStageFlags2::COMPUTE_SHADER
                    | vk::PipelineStageFlags2::FRAGMENT_SHADER
                    | vk::PipelineStageFlags2::ACCELERATION_STRUCTURE_BUILD_KHR,
            ),
            Self::Draws | Self::Clusters | Self::Particles => {
                Carried::read(vk::PipelineStageFlags2::NONE)
            }
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Carried {
    pub write: Option<BufferUse>,
    pub readers: vk::PipelineStageFlags2,
}

impl Carried {
    fn read(readers: vk::PipelineStageFlags2) -> Self {
        Self {
            write: None,
            readers,
        }
    }
}

pub(crate) const SHADER_STAGES: vk::PipelineStageFlags2 = vk::PipelineStageFlags2::from_raw(
    vk::PipelineStageFlags2::VERTEX_SHADER.as_raw()
        | vk::PipelineStageFlags2::FRAGMENT_SHADER.as_raw()
        | vk::PipelineStageFlags2::COMPUTE_SHADER.as_raw(),
);

const BUFFER_WRITES: vk::AccessFlags2 = vk::AccessFlags2::from_raw(
    vk::AccessFlags2::SHADER_STORAGE_WRITE.as_raw()
        | vk::AccessFlags2::TRANSFER_WRITE.as_raw()
        | vk::AccessFlags2::MEMORY_WRITE.as_raw()
        | vk::AccessFlags2::ACCELERATION_STRUCTURE_WRITE_KHR.as_raw(),
);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct BufferUse {
    pub stage: vk::PipelineStageFlags2,
    pub access: vk::AccessFlags2,
}

impl BufferUse {
    pub(crate) const NONE: Self = Self::new(vk::PipelineStageFlags2::NONE, vk::AccessFlags2::NONE);

    pub(crate) const fn new(stage: vk::PipelineStageFlags2, access: vk::AccessFlags2) -> Self {
        Self { stage, access }
    }

    pub(crate) fn storage_write(stage: vk::PipelineStageFlags2) -> Self {
        Self::new(stage, vk::AccessFlags2::SHADER_STORAGE_WRITE)
    }

    pub(crate) fn storage_read(stage: vk::PipelineStageFlags2) -> Self {
        Self::new(stage, vk::AccessFlags2::SHADER_STORAGE_READ)
    }

    pub(crate) fn indirect() -> Self {
        Self::new(
            vk::PipelineStageFlags2::DRAW_INDIRECT,
            vk::AccessFlags2::INDIRECT_COMMAND_READ,
        )
    }

    pub(super) fn writes(self) -> bool {
        self.access.intersects(BUFFER_WRITES)
    }
}

pub(crate) fn is_depth(format: vk::Format) -> bool {
    matches!(
        format,
        vk::Format::D32_SFLOAT | vk::Format::D16_UNORM | vk::Format::D24_UNORM_S8_UINT
    )
}

pub(crate) fn scaled(extent: vk::Extent2D, divisor: u32) -> vk::Extent2D {
    vk::Extent2D {
        width: extent.width.div_ceil(divisor).max(1),
        height: extent.height.div_ceil(divisor).max(1),
    }
}

pub(crate) fn groups(extent: vk::Extent2D, tile: u32) -> (u32, u32) {
    (extent.width.div_ceil(tile), extent.height.div_ceil(tile))
}
