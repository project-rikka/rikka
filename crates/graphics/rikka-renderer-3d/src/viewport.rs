use std::sync::Arc;
use std::time::Duration;

use ash::vk;

use crate::error::Result;
use crate::gpu::{CLUSTER_SLICES, CLUSTER_TILE, NONE_INDEX};
use crate::graph::ViewGraph;
use crate::history::ViewHistory;
use crate::recipes::ViewFormats;
use crate::reclaim::Reclaimed;
use crate::shader::Tracing;
use crate::temporal::{Antialiasing, TemporalState};
use rikka_gpu::core::device::RenderDevice;
use rikka_gpu::core::image::{GpuImage, ImageDesc, mip_levels_for};
use rikka_gpu::core::runtime::{BindPoint, Recycler};
use rikka_gpu::reclaim::Reclaimer;
use rikka_gpu::texture::TextureId;
use rikka_gpu::texture::TexturePool;

pub(crate) const COLOR_FORMAT: vk::Format = vk::Format::R16G16B16A16_SFLOAT;

pub(crate) const DEPTH_FORMAT: vk::Format = vk::Format::D32_SFLOAT;

pub(crate) const IDENTITY_FORMAT: vk::Format = vk::Format::R32_UINT;

pub(crate) const HZB_FORMAT: vk::Format = vk::Format::R32_SFLOAT;

pub(crate) const OUTLINE_FORMAT: vk::Format = vk::Format::R8_UINT;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Warming {
    #[default]
    Wait,
    DrawPlain,
}

#[derive(Clone, Copy, Debug)]
pub struct ViewportConfig {
    pub width: u32,
    pub height: u32,
    pub samples: u32,
    pub identity: bool,
    pub occlusion_culling: bool,
    pub antialiasing: Antialiasing,
    pub warming: Warming,
}

impl Default for ViewportConfig {
    fn default() -> Self {
        Self {
            width: 1,
            height: 1,
            samples: 4,
            identity: true,
            occlusion_culling: true,
            antialiasing: Antialiasing::Multisample,
            warming: Warming::Wait,
        }
    }
}

impl ViewportConfig {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width: width.max(1),
            height: height.max(1),
            ..Self::default()
        }
    }

    pub fn samples(mut self, samples: u32) -> Self {
        self.samples = samples;
        self
    }

    pub fn identity(mut self, enabled: bool) -> Self {
        self.identity = enabled;
        self
    }

    pub fn occlusion_culling(mut self, enabled: bool) -> Self {
        self.occlusion_culling = enabled;
        self
    }

    pub fn antialiasing(mut self, antialiasing: Antialiasing) -> Self {
        self.antialiasing = antialiasing;
        self
    }

    pub fn warming(mut self, warming: Warming) -> Self {
        self.warming = warming;
        self
    }

    pub(crate) fn wanted_samples(&self) -> u32 {
        match self.antialiasing {
            Antialiasing::Upscale(_) => 1,
            Antialiasing::Multisample | Antialiasing::Temporal => self.samples,
        }
    }
}

pub(crate) struct ViewportImages {
    pub color: Option<GpuImage>,
    pub resolved: GpuImage,
    pub depth: Option<GpuImage>,
    pub depth_resolved: GpuImage,
    pub identity: Option<GpuImage>,
    pub identity_resolved: Option<GpuImage>,
    pub outline: Option<GpuImage>,
    pub hzb: Option<Hzb>,
}

pub(crate) struct Targets {
    pub extent: vk::Extent2D,
    pub capacity: vk::Extent2D,
    pub images: ViewportImages,
    pub cluster_counts: [u32; 3],
    pub resolved_slot: u32,
    pub depth_resolved_slot: u32,
    pub identity_slot: u32,
    pub outline_slot: u32,
}

pub struct Viewport {
    pub(crate) config: ViewportConfig,
    pub(crate) samples: vk::SampleCountFlags,
    pub(crate) targets: Option<Targets>,
    pub(crate) output: TextureId,
    pub(crate) drawn: bool,
    pub(crate) look_pending: bool,
    pub(crate) graph: ViewGraph,
    pub(crate) history: ViewHistory,
    pub(crate) temporal: Option<TemporalState>,
    pub(crate) reclaimer: Reclaimer,
}

pub(crate) struct ViewportParts {
    pub output: TextureId,
    pub targets: Option<Targets>,
    pub graph: ViewGraph,
    pub history: ViewHistory,
    pub temporal: Option<TemporalState>,
}

impl ViewportParts {
    pub(crate) fn release(self, recycler: &mut Recycler<'_>, textures: &mut TexturePool) {
        textures.destroy(recycler, self.output);
        self.graph.release(recycler);
        self.history.release(recycler);
        if let Some(temporal) = self.temporal {
            recycler.retire.defer(recycler.value, Box::new(temporal));
        }
        if let Some(targets) = self.targets {
            targets.release(recycler);
        }
    }
}

pub(crate) struct Hzb {
    pub image: GpuImage,
    pub extent: vk::Extent2D,
    pub levels: u32,
    pub sampled_slot: u32,
    pub level_slots: Vec<u32>,
}

impl Viewport {
    pub fn width(&self) -> u32 {
        self.config.width
    }

    pub fn height(&self) -> u32 {
        self.config.height
    }

    pub fn texture(&self) -> TextureId {
        self.output
    }

    pub fn samples(&self) -> u32 {
        self.samples.as_raw()
    }

    pub fn antialiasing(&self) -> Antialiasing {
        self.config.antialiasing
    }

    pub fn render_size(&self) -> (u32, u32) {
        let extent = self.extent();
        (extent.width, extent.height)
    }

    pub fn render_scale(&self) -> f32 {
        self.temporal.as_ref().map_or(1.0, TemporalState::scale)
    }

    pub fn gpu_time(&self) -> Option<Duration> {
        self.temporal.as_ref().and_then(TemporalState::measured)
    }

    pub fn is_converging(&self) -> bool {
        self.look_pending
            || self
                .temporal
                .as_ref()
                .is_some_and(|temporal| !temporal.is_settled())
    }

    pub(crate) fn targets(&self) -> &Targets {
        self.targets
            .as_ref()
            .expect("viewport targets exist until destroy")
    }

    pub(crate) fn cluster_count(&self) -> u32 {
        let counts = self.targets().cluster_counts;
        counts[0] * counts[1] * counts[2]
    }

    pub(crate) fn extent(&self) -> vk::Extent2D {
        self.targets().extent
    }

    pub(crate) fn capacity(&self) -> vk::Extent2D {
        self.targets().capacity
    }

    pub(crate) fn display_extent(&self) -> vk::Extent2D {
        vk::Extent2D {
            width: self.config.width,
            height: self.config.height,
        }
    }

    pub(crate) fn formats(&self) -> ViewFormats {
        ViewFormats {
            samples: self.samples,
            identity: self.targets().identity_slot != NONE_INDEX,
            emission: false,
            tracing: Tracing::Raster,
        }
    }

    pub(crate) fn multisampled(&self) -> bool {
        self.samples != vk::SampleCountFlags::TYPE_1
    }

    pub(crate) fn hzb_ref(&self) -> Option<&Hzb> {
        self.targets().images.hzb.as_ref()
    }

    pub(crate) fn retarget(&mut self, completed: u64) {
        let Some(temporal) = &mut self.temporal else {
            return;
        };
        temporal.observe(completed);
        let wanted = temporal.render_extent();
        let targets = self
            .targets
            .as_mut()
            .expect("viewport targets exist until destroy");
        targets.extent = wanted;
        targets.cluster_counts = cluster_counts(wanted);
    }
}

impl Targets {
    pub(crate) fn create(
        bind: &mut BindPoint<'_>,
        config: &ViewportConfig,
        samples: vk::SampleCountFlags,
        capacity: vk::Extent2D,
    ) -> Result<Self> {
        let device = bind.device;
        let Attachments {
            resolved,
            depth_resolved,
            color,
            depth,
            identity,
            identity_resolved,
            outline,
        } = attachment_descs(config, samples, capacity);
        let create = |name: &'static str, desc: Option<ImageDesc>| {
            desc.map(|desc| GpuImage::create(device.clone(), name, &desc))
                .transpose()
        };
        let resolved = GpuImage::create(device.clone(), "rikka.three.color", &resolved)?;
        let depth_resolved =
            GpuImage::create(device.clone(), "rikka.three.depth", &depth_resolved)?;
        let color = create("rikka.three.color_ms", color)?;
        let depth = create("rikka.three.depth_ms", depth)?;
        let identity = create("rikka.three.identity_ms", identity)?;
        let identity_resolved = create("rikka.three.identity", identity_resolved)?;
        let outline = create("rikka.three.outline", outline)?;

        let resolved_slot = bind
            .resources
            .allocate(resolved.view(), bind.samplers.linear_clamp())?;
        let depth_resolved_slot = bind
            .resources
            .allocate(depth_resolved.view(), bind.samplers.depth_reduce())?;
        let nearest = |bind: &mut BindPoint<'_>, image: &Option<GpuImage>| match image {
            Some(image) => bind
                .resources
                .allocate(image.view(), bind.samplers.nearest_clamp()),
            None => Ok(NONE_INDEX),
        };
        let identity_slot = nearest(bind, &identity_resolved)?;
        let outline_slot = nearest(bind, &outline)?;
        let hzb = match config.occlusion_culling {
            true => Some(create_hzb(device, bind, capacity)?),
            false => None,
        };
        Ok(Self {
            extent: capacity,
            capacity,
            images: ViewportImages {
                color,
                resolved,
                depth,
                depth_resolved,
                identity,
                identity_resolved,
                outline,
                hzb,
            },
            cluster_counts: cluster_counts(capacity),
            resolved_slot,
            depth_resolved_slot,
            identity_slot,
            outline_slot,
        })
    }

    pub(crate) fn release(self, recycler: &mut Recycler<'_>) {
        let value = recycler.value;
        recycler.resources.release(self.resolved_slot, value);
        recycler.resources.release(self.depth_resolved_slot, value);
        for slot in [self.identity_slot, self.outline_slot] {
            if slot != NONE_INDEX {
                recycler.resources.release(slot, value);
            }
        }
        if let Some(hzb) = &self.images.hzb {
            recycler.resources.release(hzb.sampled_slot, value);
            for &slot in &hzb.level_slots {
                recycler.resources.release_storage(slot, value);
            }
        }
        recycler.retire.defer(value, Box::new(self.images));
    }
}

impl ViewportImages {
    pub(crate) fn color_target(&mut self) -> &mut GpuImage {
        match &mut self.color {
            Some(color) => color,
            None => &mut self.resolved,
        }
    }

    pub(crate) fn depth_target(&mut self) -> &mut GpuImage {
        match &mut self.depth {
            Some(depth) => depth,
            None => &mut self.depth_resolved,
        }
    }

    pub(crate) fn identity_target(&mut self) -> Option<&mut GpuImage> {
        match self.identity.is_some() {
            true => self.identity.as_mut(),
            false => self.identity_resolved.as_mut(),
        }
    }
}

impl Drop for Viewport {
    fn drop(&mut self) {
        self.reclaimer
            .route(Reclaimed::Viewport(Box::new(ViewportParts {
                output: self.output,
                targets: self.targets.take(),
                graph: std::mem::take(&mut self.graph),
                history: std::mem::take(&mut self.history),
                temporal: self.temporal.take(),
            })));
    }
}

impl std::fmt::Debug for Viewport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Viewport")
            .field("width", &self.config.width)
            .field("height", &self.config.height)
            .field("samples", &self.samples)
            .field("antialiasing", &self.config.antialiasing)
            .finish_non_exhaustive()
    }
}

pub(crate) fn cluster_counts(extent: vk::Extent2D) -> [u32; 3] {
    [
        extent.width.div_ceil(CLUSTER_TILE).max(1),
        extent.height.div_ceil(CLUSTER_TILE).max(1),
        CLUSTER_SLICES,
    ]
}

pub(crate) fn hzb_extent(extent: vk::Extent2D) -> vk::Extent2D {
    vk::Extent2D {
        width: previous_power_of_two(extent.width),
        height: previous_power_of_two(extent.height),
    }
}

fn previous_power_of_two(value: u32) -> u32 {
    let value = value.max(1);
    1 << (31 - value.leading_zeros())
}

pub(crate) fn create_hzb(
    device: &Arc<RenderDevice>,
    bind: &mut BindPoint<'_>,
    extent: vk::Extent2D,
) -> Result<Hzb> {
    let extent = hzb_extent(extent);
    let levels = mip_levels_for(extent);
    let mut image = GpuImage::create(
        device.clone(),
        "rikka.three.hzb",
        &ImageDesc::color(
            extent,
            HZB_FORMAT,
            vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::STORAGE,
        )
        .mips(levels),
    )?;

    let sampled_slot = bind.resources.allocate_in(
        image.view(),
        bind.samplers.depth_reduce(),
        vk::ImageLayout::GENERAL,
    )?;
    let mut level_slots = Vec::with_capacity(levels as usize);
    for level in 0..levels {
        let view = image.mip_view(level)?;
        level_slots.push(bind.resources.allocate_storage(view)?);
    }

    Ok(Hzb {
        image,
        extent,
        levels,
        sampled_slot,
        level_slots,
    })
}

pub(crate) struct Attachments {
    pub resolved: ImageDesc,
    pub depth_resolved: ImageDesc,
    pub color: Option<ImageDesc>,
    pub depth: Option<ImageDesc>,
    pub identity: Option<ImageDesc>,
    pub identity_resolved: Option<ImageDesc>,
    pub outline: Option<ImageDesc>,
}

const TRANSIENT: vk::ImageUsageFlags = vk::ImageUsageFlags::TRANSIENT_ATTACHMENT;

pub(crate) fn attachment_descs(
    config: &ViewportConfig,
    samples: vk::SampleCountFlags,
    extent: vk::Extent2D,
) -> Attachments {
    let multisampled = samples != vk::SampleCountFlags::TYPE_1;
    let sampled_color = vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED;

    let resolved = ImageDesc::color(extent, COLOR_FORMAT, sampled_color);
    let color = multisampled.then(|| {
        ImageDesc::color(
            extent,
            COLOR_FORMAT,
            vk::ImageUsageFlags::COLOR_ATTACHMENT | TRANSIENT,
        )
        .samples(samples)
    });

    let depth_resolved = ImageDesc::depth(
        extent,
        DEPTH_FORMAT,
        vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
    );
    let depth = multisampled.then(|| {
        ImageDesc::depth(
            extent,
            DEPTH_FORMAT,
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
        )
        .samples(samples)
    });

    let identity_resolved = config
        .identity
        .then(|| ImageDesc::color(extent, IDENTITY_FORMAT, sampled_color));
    let identity = (config.identity && multisampled).then(|| {
        ImageDesc::color(
            extent,
            IDENTITY_FORMAT,
            vk::ImageUsageFlags::COLOR_ATTACHMENT,
        )
        .samples(samples)
    });
    let outline = config
        .identity
        .then(|| ImageDesc::color(extent, OUTLINE_FORMAT, sampled_color));

    Attachments {
        resolved,
        depth_resolved,
        color,
        depth,
        identity,
        identity_resolved,
        outline,
    }
}
