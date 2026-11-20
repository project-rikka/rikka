use std::ptr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ash::vk;
use rikka_annexb::NalSyntax;
use rikka_dmabuf_convert::{Converter, Imported};
use rikka_gpu::{DmabufSpec, Gpu};

use crate::Codec;
use crate::capabilities::Capabilities;
use crate::convert::Target;
use crate::device::VideoDevice;
use crate::error::{Error, Result};
use crate::picture::{Dpb, Image, ImageRequest};
use crate::profile::Profile;
use crate::ring::Ring;
use crate::session::{Session, SessionRequest};
use crate::syntax::{Layout, Plan, RateChain, RateSettings, Syntax};

const STATUS_COMPLETE: i32 = 1;
const STATUS_OVERFLOW: i32 = -1_000_299_000;
const SPARE_BITSTREAM: u64 = 1 << 20;
const HEAVY_PIXEL_RATE: u64 = 2560 * 1440 * 60;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rate {
    pub bitrate: u64,
    pub buffer: Duration,
    pub min_qp: u8,
    pub max_qp: u8,
    pub intra_burst: Duration,
    pub predicted_burst: Duration,
}

impl Default for Rate {
    fn default() -> Rate {
        Rate {
            bitrate: 10_000_000,
            buffer: Duration::from_millis(100),
            min_qp: 0,
            max_qp: 51,
            intra_burst: Duration::from_millis(250),
            predicted_burst: Duration::from_millis(50),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quality {
    Rate,
    Constant(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Request {
    reference: Option<u32>,
    quality: Quality,
}

impl Request {
    #[must_use]
    pub fn intra() -> Request {
        Request {
            reference: None,
            quality: Quality::Rate,
        }
    }

    #[must_use]
    pub fn predicted(reference: u32) -> Request {
        Request {
            reference: Some(reference),
            quality: Quality::Rate,
        }
    }

    #[must_use]
    pub fn constant(self, qp: u8) -> Request {
        Request {
            quality: Quality::Constant(qp),
            ..self
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Encoded {
    pub frame: u32,
    pub reference: u32,
    pub intra: bool,
    pub data: Vec<u8>,
    pub took: Duration,
}

impl std::fmt::Debug for Encoded {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Encoded")
            .field("frame", &self.frame)
            .field("reference", &self.reference)
            .field("intra", &self.intra)
            .field("bytes", &self.data.len())
            .field("took", &self.took)
            .finish()
    }
}

#[derive(Debug)]
#[must_use]
pub struct EncoderBuilder<'a> {
    gpu: &'a Gpu,
    codec: Codec,
    width: u32,
    height: u32,
    frame_rate: u32,
    references: u8,
    rate: Rate,
}

impl EncoderBuilder<'_> {
    pub fn frame_rate(self, frame_rate: u32) -> Self {
        EncoderBuilder {
            frame_rate: frame_rate.max(1),
            ..self
        }
    }

    pub fn references(self, references: u8) -> Self {
        EncoderBuilder {
            references: references.max(1),
            ..self
        }
    }

    pub fn rate(self, rate: Rate) -> Self {
        EncoderBuilder { rate, ..self }
    }

    pub fn build(self) -> Result<Encoder> {
        Encoder::open(self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Applied {
    mode: vk::VideoEncodeRateControlModeFlagsKHR,
    rate: Rate,
}

pub struct Encoder {
    codec: Codec,
    picture: vk::Extent2D,
    coded: vk::Extent2D,
    frame_rate: u32,
    capabilities: Capabilities,
    syntax: Syntax,
    ring: Ring,
    rate: Rate,
    rated_mode: vk::VideoEncodeRateControlModeFlagsKHR,
    applied: Option<Applied>,
    quality: u32,
    references_ready: bool,
    converter: Converter,
    timeline: vk::Semaphore,
    timeline_value: u64,
    fence: vk::Fence,
    commands: vk::CommandPool,
    command: vk::CommandBuffer,
    queries: vk::QueryPool,
    bitstream: vk::Buffer,
    bitstream_memory: vk::DeviceMemory,
    bitstream_size: u64,
    bitstream_mapped: *const u8,
    bitstream_coherent: bool,
    dpb: Dpb,
    input: Image,
    session: Session,
    profile: Profile,
    name: String,
    device: Arc<VideoDevice>,
}

unsafe impl Send for Encoder {}

impl Encoder {
    pub fn builder(gpu: &Gpu, codec: Codec, width: u32, height: u32) -> EncoderBuilder<'_> {
        EncoderBuilder {
            gpu,
            codec,
            width,
            height,
            frame_rate: 60,
            references: Syntax::most_references(codec),
            rate: Rate::default(),
        }
    }

    pub fn supports(gpu: &Gpu, codec: Codec) -> bool {
        let Ok(device) = VideoDevice::open(gpu) else {
            return false;
        };
        Profile::negotiate(&device, codec).is_ok()
    }

    fn open(builder: EncoderBuilder<'_>) -> Result<Encoder> {
        let device = VideoDevice::open(builder.gpu)?;
        let (profile, capabilities) = Profile::negotiate(&device, builder.codec)?;
        let picture = vk::Extent2D {
            width: builder.width & !1,
            height: builder.height & !1,
        };
        let coded = Syntax::coded(builder.codec, picture.width, picture.height, &capabilities);
        if coded.width > capabilities.max_extent.width
            || coded.height > capabilities.max_extent.height
            || coded.width < capabilities.min_extent.width
            || coded.height < capabilities.min_extent.height
        {
            return Err(Error::Unsupported(format!(
                "{} pictures of {}x{}",
                builder.codec, picture.width, picture.height
            )));
        }
        let slots_cap = capabilities.dpb_slots.saturating_sub(1).max(1);
        let references = u32::from(builder.references)
            .min(slots_cap)
            .min(u32::from(Syntax::most_references(builder.codec))) as u8;
        let layout = Layout {
            picture,
            coded,
            references,
            frame_rate: builder.frame_rate,
        };
        let syntax = Syntax::new(layout, &capabilities);
        let pixel_rate =
            u64::from(coded.width) * u64::from(coded.height) * u64::from(builder.frame_rate);
        let quality = match pixel_rate > HEAVY_PIXEL_RATE {
            true => (capabilities.quality_levels - 1) / 3,
            false => (capabilities.quality_levels - 1) / 2,
        };
        let session = Session::new(
            &device,
            &SessionRequest {
                profile: &profile,
                capabilities: &capabilities,
                syntax: &syntax,
                coded,
                slots: u32::from(references) + 1,
                quality,
            },
        )?;
        let input = Image::new(
            &device,
            &ImageRequest {
                profile: &profile,
                format: capabilities.picture_format,
                extent: coded,
                layers: 1,
                usage: vk::ImageUsageFlags::VIDEO_ENCODE_SRC_KHR | vk::ImageUsageFlags::STORAGE,
                view_usage: vk::ImageUsageFlags::VIDEO_ENCODE_SRC_KHR,
                flags: vk::ImageCreateFlags::MUTABLE_FORMAT | vk::ImageCreateFlags::EXTENDED_USAGE,
                shared: true,
            },
        )?;
        let dpb = Dpb::new(
            &device,
            &capabilities,
            &profile,
            coded,
            u32::from(references) + 1,
        )?;
        let converter = Converter::new(builder.gpu.compute()?)?;
        let rated_mode = [
            vk::VideoEncodeRateControlModeFlagsKHR::CBR,
            vk::VideoEncodeRateControlModeFlagsKHR::VBR,
        ]
        .into_iter()
        .find(|mode| capabilities.rate_modes.contains(*mode))
        .unwrap_or(vk::VideoEncodeRateControlModeFlagsKHR::DEFAULT);
        let name = format!("{} (Vulkan Video {})", device.name, builder.codec);
        let mut encoder = Encoder {
            codec: builder.codec,
            picture,
            coded,
            frame_rate: builder.frame_rate,
            ring: Ring::new(references),
            rate: builder.rate,
            rated_mode,
            applied: None,
            quality,
            references_ready: false,
            converter,
            timeline: vk::Semaphore::null(),
            timeline_value: 0,
            fence: vk::Fence::null(),
            commands: vk::CommandPool::null(),
            command: vk::CommandBuffer::null(),
            queries: vk::QueryPool::null(),
            bitstream: vk::Buffer::null(),
            bitstream_memory: vk::DeviceMemory::null(),
            bitstream_size: 0,
            bitstream_mapped: ptr::null(),
            bitstream_coherent: true,
            dpb,
            input,
            session,
            profile,
            capabilities,
            syntax,
            name,
            device,
        };
        encoder.create_objects()?;
        tracing::info!(
            encoder = encoder.name,
            width = picture.width,
            height = picture.height,
            frame_rate = builder.frame_rate,
            references,
            quality,
            "opened a Vulkan Video encoder"
        );
        Ok(encoder)
    }

    fn create_objects(&mut self) -> Result<()> {
        let device = &self.device.device;
        let mut timeline_type = vk::SemaphoreTypeCreateInfo::default()
            .semaphore_type(vk::SemaphoreType::TIMELINE)
            .initial_value(0);
        let semaphore = vk::SemaphoreCreateInfo::default().push_next(&mut timeline_type);
        self.timeline = unsafe { device.create_semaphore(&semaphore, None) }?;
        self.fence = unsafe { device.create_fence(&vk::FenceCreateInfo::default(), None) }?;
        let pool = vk::CommandPoolCreateInfo::default()
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
            .queue_family_index(self.device.encode_family);
        self.commands = unsafe { device.create_command_pool(&pool, None) }?;
        let allocate = vk::CommandBufferAllocateInfo::default()
            .command_pool(self.commands)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);
        self.command = unsafe { device.allocate_command_buffers(&allocate) }?[0];
        let mut feedback = vk::QueryPoolVideoEncodeFeedbackCreateInfoKHR::default()
            .encode_feedback_flags(
                vk::VideoEncodeFeedbackFlagsKHR::BITSTREAM_BUFFER_OFFSET
                    | vk::VideoEncodeFeedbackFlagsKHR::BITSTREAM_BYTES_WRITTEN,
            );
        feedback.p_next = ptr::from_ref(self.profile.info()).cast();
        let mut queries = vk::QueryPoolCreateInfo::default()
            .query_type(vk::QueryType::VIDEO_ENCODE_FEEDBACK_KHR)
            .query_count(1);
        queries.p_next = ptr::from_ref(&feedback).cast();
        self.queries = unsafe { device.create_query_pool(&queries, None) }?;
        let raw = u64::from(self.coded.width) * u64::from(self.coded.height) * 3 / 2;
        self.bitstream_size =
            (raw + SPARE_BITSTREAM).next_multiple_of(self.capabilities.size_alignment);
        let mut buffer = vk::BufferCreateInfo::default()
            .size(self.bitstream_size)
            .usage(vk::BufferUsageFlags::VIDEO_ENCODE_DST_KHR)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        buffer.p_next = ptr::from_ref(self.profile.list()).cast();
        self.bitstream = unsafe { device.create_buffer(&buffer, None) }?;
        let requirements = unsafe { device.get_buffer_memory_requirements(self.bitstream) };
        let cached = vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_CACHED;
        let coherent =
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT;
        let index = self
            .device
            .memory_type(requirements.memory_type_bits, cached, coherent)?;
        self.bitstream_coherent = self.device.memory.memory_types[index as usize]
            .property_flags
            .contains(vk::MemoryPropertyFlags::HOST_COHERENT);
        self.bitstream_memory = self.device.allocate(requirements, cached, coherent)?;
        unsafe { device.bind_buffer_memory(self.bitstream, self.bitstream_memory, 0) }?;
        self.bitstream_mapped = unsafe {
            device.map_memory(
                self.bitstream_memory,
                0,
                vk::WHOLE_SIZE,
                vk::MemoryMapFlags::empty(),
            )
        }?
        .cast();
        Ok(())
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn codec(&self) -> Codec {
        self.codec
    }

    #[must_use]
    pub fn size(&self) -> (u32, u32) {
        (self.picture.width, self.picture.height)
    }

    #[must_use]
    pub fn frame_rate(&self) -> u32 {
        self.frame_rate
    }

    #[must_use]
    pub fn next_frame(&self) -> u32 {
        self.ring.next_frame()
    }

    #[must_use]
    pub fn parameter_sets(&self) -> &[u8] {
        &self.session.header
    }

    #[must_use]
    pub fn holds(&self, frame: u32) -> bool {
        self.ring.holds(frame)
    }

    pub fn import(&self, spec: &DmabufSpec<'_>) -> Result<Imported> {
        Ok(self.converter.import(spec)?)
    }

    pub fn set_rate(&mut self, rate: Rate) {
        self.rate = rate;
    }

    pub fn encode(&mut self, source: &Imported, request: Request) -> Result<Encoded> {
        let started = Instant::now();
        let qp = match request.quality {
            Quality::Constant(qp)
                if self
                    .capabilities
                    .rate_modes
                    .contains(vk::VideoEncodeRateControlModeFlagsKHR::DISABLED) =>
            {
                let (lowest, highest) = self.capabilities.qp_range();
                Some(i32::from(qp).clamp(lowest, highest))
            }
            _ => None,
        };
        let plan = self.ring.plan(request.reference, qp)?;
        self.timeline_value += 1;
        let target = Target {
            image: self.input.image,
            surface: (self.coded.width, self.coded.height),
            signal: (self.timeline, self.timeline_value),
        };
        self.converter.convert(
            source,
            target.written(),
            (self.picture.width, self.picture.height),
        )?;
        let wanted = match qp {
            Some(_) => Applied {
                mode: vk::VideoEncodeRateControlModeFlagsKHR::DISABLED,
                rate: self.rate,
            },
            None => Applied {
                mode: self.rated_mode,
                rate: self.rate,
            },
        };
        self.record(&plan, wanted)?;
        self.submit()?;
        let bytes = self.read_bitstream()?;
        let mut data = Vec::with_capacity(bytes.len() + self.session.header.len());
        if plan.intra {
            data.extend_from_slice(&self.session.header);
        }
        rikka_annexb::append_without_filler(bytes, nal_syntax(self.codec), &mut data);
        let frame = self.ring.commit(&plan);
        self.applied = Some(wanted);
        self.references_ready = true;
        let reference = match plan.reference {
            Some(reference) => frame - plan.order + reference.order,
            None => frame,
        };
        Ok(Encoded {
            frame,
            reference,
            intra: plan.intra,
            data,
            took: started.elapsed(),
        })
    }

    fn rate_parts(&self, applied: Applied) -> RateParts {
        if applied.mode == vk::VideoEncodeRateControlModeFlagsKHR::DISABLED {
            let info = vk::VideoEncodeRateControlInfoKHR::default()
                .rate_control_mode(vk::VideoEncodeRateControlModeFlagsKHR::DISABLED);
            return RateParts {
                info: Box::new(info),
                _layer: Box::new(vk::VideoEncodeRateControlLayerInfoKHR::default()),
                _codec: None,
            };
        }
        if applied.mode == vk::VideoEncodeRateControlModeFlagsKHR::DEFAULT {
            return RateParts {
                info: Box::new(
                    vk::VideoEncodeRateControlInfoKHR::default()
                        .rate_control_mode(vk::VideoEncodeRateControlModeFlagsKHR::DEFAULT),
                ),
                _layer: Box::new(vk::VideoEncodeRateControlLayerInfoKHR::default()),
                _codec: None,
            };
        }
        let rate = applied.rate;
        let (lowest, highest) = self.capabilities.qp_range();
        let bytes_for = |span: Duration| {
            ((rate.bitrate as f64 * span.as_secs_f64() / 8.0) as u64).clamp(1, u64::from(u32::MAX))
                as u32
        };
        let settings = RateSettings {
            min_qp: i32::from(rate.min_qp).clamp(lowest, highest),
            max_qp: i32::from(rate.max_qp).clamp(lowest, highest),
            max_intra_bytes: bytes_for(rate.intra_burst),
            max_predicted_bytes: bytes_for(rate.predicted_burst),
        };
        let codec = self.syntax.rate(&settings);
        let mut layer = Box::new(
            vk::VideoEncodeRateControlLayerInfoKHR::default()
                .average_bitrate(rate.bitrate.max(1))
                .max_bitrate(rate.bitrate.max(1))
                .frame_rate_numerator(self.frame_rate)
                .frame_rate_denominator(1),
        );
        layer.p_next = codec.layer();
        let buffer = (rate.buffer.as_millis() as u32).max(1);
        let mut info = Box::new(
            vk::VideoEncodeRateControlInfoKHR::default()
                .rate_control_mode(applied.mode)
                .virtual_buffer_size_in_ms(buffer)
                .initial_virtual_buffer_size_in_ms(buffer / 2),
        );
        info.layer_count = 1;
        info.p_layers = ptr::from_ref(layer.as_ref());
        info.p_next = codec.info();
        RateParts {
            info,
            _layer: layer,
            _codec: Some(codec),
        }
    }

    fn record(&mut self, plan: &Plan, wanted: Applied) -> Result<()> {
        let device = &self.device.device;
        let command = self.command;
        unsafe {
            device.reset_command_buffer(command, vk::CommandBufferResetFlags::empty())?;
            device.begin_command_buffer(
                command,
                &vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )?;
            device.cmd_reset_query_pool(command, self.queries, 0, 1);
        }
        let mut barriers = vec![
            vk::ImageMemoryBarrier2::default()
                .src_stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS)
                .dst_stage_mask(vk::PipelineStageFlags2::VIDEO_ENCODE_KHR)
                .dst_access_mask(vk::AccessFlags2::VIDEO_ENCODE_READ_KHR)
                .old_layout(vk::ImageLayout::GENERAL)
                .new_layout(vk::ImageLayout::VIDEO_ENCODE_SRC_KHR)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .image(self.input.image)
                .subresource_range(color_range(1)),
        ];
        if !self.references_ready {
            let layers = match self.capabilities.separate_references {
                true => 1,
                false => self.dpb.slots.len() as u32,
            };
            barriers.extend(self.dpb.images().map(|image| {
                vk::ImageMemoryBarrier2::default()
                    .src_stage_mask(vk::PipelineStageFlags2::NONE)
                    .dst_stage_mask(vk::PipelineStageFlags2::VIDEO_ENCODE_KHR)
                    .dst_access_mask(
                        vk::AccessFlags2::VIDEO_ENCODE_READ_KHR
                            | vk::AccessFlags2::VIDEO_ENCODE_WRITE_KHR,
                    )
                    .old_layout(vk::ImageLayout::UNDEFINED)
                    .new_layout(vk::ImageLayout::VIDEO_ENCODE_DPB_KHR)
                    .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .image(image)
                    .subresource_range(color_range(layers))
            }));
        }
        unsafe {
            device.cmd_pipeline_barrier2(
                command,
                &vk::DependencyInfo::default().image_memory_barriers(&barriers),
            );
        }
        let picture = self.syntax.picture(plan);
        let resource = |view: vk::ImageView| {
            vk::VideoPictureResourceInfoKHR::default()
                .coded_extent(self.coded)
                .image_view_binding(view)
        };
        let setup_slot = &self.dpb.slots[usize::from(plan.setup_slot)];
        let setup_resource = resource(setup_slot.view);
        let reference_resource = plan
            .reference
            .map(|reference| resource(self.dpb.slots[usize::from(reference.slot)].view));
        let mut bound = vec![vk::VideoReferenceSlotInfoKHR {
            slot_index: -1,
            p_picture_resource: &setup_resource,
            ..Default::default()
        }];
        if let (Some(reference), Some(resource)) = (plan.reference, &reference_resource) {
            bound.push(vk::VideoReferenceSlotInfoKHR {
                slot_index: i32::from(reference.slot),
                p_picture_resource: resource,
                ..Default::default()
            });
        }
        let current = self.applied.map(|applied| self.rate_parts(applied));
        let mut begin = vk::VideoBeginCodingInfoKHR::default()
            .video_session(self.session.session)
            .video_session_parameters(self.session.parameters)
            .reference_slots(&bound);
        if let Some(current) = &current {
            begin.p_next = ptr::from_ref(current.info.as_ref()).cast();
        }
        let fns = &self.device.queue_fns;
        unsafe { (fns.cmd_begin_video_coding_khr)(command, &begin) };
        let first = self.applied.is_none();
        if first || self.applied != Some(wanted) {
            let next = self.rate_parts(wanted);
            let mut quality =
                vk::VideoEncodeQualityLevelInfoKHR::default().quality_level(self.quality);
            quality.p_next = ptr::from_ref(next.info.as_ref()).cast();
            let (flags, chain): (_, *const std::ffi::c_void) = match first {
                true => (
                    vk::VideoCodingControlFlagsKHR::RESET
                        | vk::VideoCodingControlFlagsKHR::ENCODE_RATE_CONTROL
                        | vk::VideoCodingControlFlagsKHR::ENCODE_QUALITY_LEVEL,
                    ptr::from_ref(&quality).cast(),
                ),
                false => (
                    vk::VideoCodingControlFlagsKHR::ENCODE_RATE_CONTROL,
                    ptr::from_ref(next.info.as_ref()).cast(),
                ),
            };
            let control = vk::VideoCodingControlInfoKHR {
                p_next: chain,
                flags,
                ..Default::default()
            };
            unsafe { (fns.cmd_control_video_coding_khr)(command, &control) };
            drop(next);
        }
        let setup = vk::VideoReferenceSlotInfoKHR {
            p_next: picture.setup(),
            slot_index: i32::from(plan.setup_slot),
            p_picture_resource: &setup_resource,
            ..Default::default()
        };
        let references: Vec<vk::VideoReferenceSlotInfoKHR<'_>> =
            match (plan.reference, &reference_resource) {
                (Some(reference), Some(resource)) => vec![vk::VideoReferenceSlotInfoKHR {
                    p_next: picture.reference(),
                    slot_index: i32::from(reference.slot),
                    p_picture_resource: resource,
                    ..Default::default()
                }],
                _ => Vec::new(),
            };
        let mut encode = vk::VideoEncodeInfoKHR::default()
            .dst_buffer(self.bitstream)
            .dst_buffer_offset(0)
            .dst_buffer_range(self.bitstream_size)
            .src_picture_resource(resource(self.input.views[0]))
            .setup_reference_slot(&setup)
            .reference_slots(&references);
        encode.p_next = picture.info();
        unsafe {
            device.cmd_begin_query(command, self.queries, 0, vk::QueryControlFlags::empty());
            (self.device.encode_fns.cmd_encode_video_khr)(command, &encode);
            device.cmd_end_query(command, self.queries, 0);
            (fns.cmd_end_video_coding_khr)(command, &vk::VideoEndCodingInfoKHR::default());
            device.end_command_buffer(command)?;
        }
        drop(current);
        drop(picture);
        Ok(())
    }

    fn submit(&mut self) -> Result<()> {
        let device = &self.device.device;
        let waits = [vk::SemaphoreSubmitInfo::default()
            .semaphore(self.timeline)
            .value(self.timeline_value)
            .stage_mask(vk::PipelineStageFlags2::VIDEO_ENCODE_KHR)];
        let commands = [vk::CommandBufferSubmitInfo::default().command_buffer(self.command)];
        let submit = vk::SubmitInfo2::default()
            .wait_semaphore_infos(&waits)
            .command_buffer_infos(&commands);
        unsafe {
            device.queue_submit2(self.device.queue, &[submit], self.fence)?;
            device.wait_for_fences(&[self.fence], true, u64::MAX)?;
            device.reset_fences(&[self.fence])?;
        }
        Ok(())
    }

    fn read_bitstream(&self) -> Result<&[u8]> {
        let mut results = [[0_u32; 3]; 1];
        unsafe {
            self.device.device.get_query_pool_results(
                self.queries,
                0,
                &mut results,
                vk::QueryResultFlags::WAIT | vk::QueryResultFlags::WITH_STATUS_KHR,
            )
        }?;
        let [offset, written, status] = results[0];
        match status as i32 {
            STATUS_COMPLETE => {}
            STATUS_OVERFLOW => return Err(Error::Overflow(self.bitstream_size)),
            other => return Err(Error::Status(other)),
        }
        let end = u64::from(offset) + u64::from(written);
        if end > self.bitstream_size {
            return Err(Error::Overflow(self.bitstream_size));
        }
        if !self.bitstream_coherent {
            let range = vk::MappedMemoryRange::default()
                .memory(self.bitstream_memory)
                .offset(0)
                .size(vk::WHOLE_SIZE);
            unsafe { self.device.device.invalidate_mapped_memory_ranges(&[range]) }?;
        }
        Ok(unsafe {
            std::slice::from_raw_parts(self.bitstream_mapped.add(offset as usize), written as usize)
        })
    }
}

struct RateParts {
    info: Box<vk::VideoEncodeRateControlInfoKHR<'static>>,
    _layer: Box<vk::VideoEncodeRateControlLayerInfoKHR<'static>>,
    _codec: Option<Box<dyn RateChain>>,
}

fn nal_syntax(codec: Codec) -> NalSyntax {
    match codec {
        Codec::H264 => NalSyntax::H264,
        Codec::H265 => NalSyntax::H265,
    }
}

fn color_range(layers: u32) -> vk::ImageSubresourceRange {
    vk::ImageSubresourceRange {
        aspect_mask: vk::ImageAspectFlags::COLOR,
        base_mip_level: 0,
        level_count: 1,
        base_array_layer: 0,
        layer_count: layers,
    }
}

impl std::fmt::Debug for Encoder {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Encoder")
            .field("name", &self.name)
            .field("picture", &self.picture)
            .field("coded", &self.coded)
            .finish_non_exhaustive()
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        let device = &self.device.device;
        unsafe {
            if let Err(error) = device.queue_wait_idle(self.device.queue) {
                tracing::warn!("the encode queue did not settle: {error}");
            }
            device.unmap_memory(self.bitstream_memory);
            device.destroy_buffer(self.bitstream, None);
            device.free_memory(self.bitstream_memory, None);
            device.destroy_query_pool(self.queries, None);
            device.destroy_command_pool(self.commands, None);
            device.destroy_fence(self.fence, None);
            device.destroy_semaphore(self.timeline, None);
        }
    }
}
