use std::any::Any;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Weak};

use ash::vk;
use rikka_spirv::Shader;
use rikka_sync::Mutex;
use rustc_hash::FxHashMap;

use crate::compute::batch::Batch;
use crate::compute::buffer::ComputeBuffer;
use crate::compute::completion::Completions;
use crate::compute::foreign::{ForeignImage, VideoTarget};
use crate::compute::format::ImageFormat;
use crate::compute::image::{ComputeImage, ComputeTile};
use crate::compute::kernel::{Kernel, KernelInner};
use crate::compute::lane::{Lane, Line, Marks, reached};
use crate::compute::pool::{Eviction, Pool};
use crate::compute::slots::Slots;
use crate::compute::table::Table;
use crate::core::device::RenderDevice;
use crate::core::recorder::Recorder;
#[cfg(unix)]
use crate::drm::DmabufSpec;
use crate::error::{Capability, Error, Result};

#[derive(Clone)]
pub struct Compute {
    pub(crate) shared: Arc<Shared>,
    pub(crate) lane: Lane,
}

pub struct Shared {
    pub device: Arc<RenderDevice>,
    pub(crate) lines: [Line; 2],
    pub(crate) table: Table,
    pub(crate) shared_by: Option<[u32; 2]>,
    supported: Vec<ImageFormat>,
    pub(crate) state: Mutex<State>,
    pub(crate) submitted: Mutex<Marks>,
    recorders: Mutex<Vec<(Lane, Recorder)>>,
    pub(crate) kernels: Mutex<FxHashMap<u64, Weak<KernelInner>>>,
    pub(crate) completions: Completions,
    minted: AtomicU64,
}

pub(crate) struct State {
    pub(crate) slots: Slots,
    retiring: Vec<(Marks, Box<dyn Any + Send>)>,
    pub(crate) pool: Pool,
}

impl Compute {
    pub fn new(device: Arc<RenderDevice>) -> Result<Compute> {
        if !device.computes() {
            return Err(Error::NotEnabled(Capability::Compute));
        }
        let table = Table::new(device.clone())?;
        let lines = [
            Line::new(&device, Lane::Front)?,
            Line::new(&device, Lane::Back)?,
        ];
        let completions = Completions::new(
            &device,
            [
                Arc::clone(&lines[0].timeline),
                Arc::clone(&lines[1].timeline),
            ],
        )?;
        let shared_by =
            (lines[0].family != lines[1].family).then_some([lines[0].family, lines[1].family]);
        let slots = Slots::new(table.capacity());
        let supported = ImageFormat::ALL
            .into_iter()
            .filter(|format| computable(&device, *format))
            .collect();
        Ok(Compute {
            shared: Arc::new(Shared {
                device,
                lines,
                table,
                shared_by,
                supported,
                state: Mutex::new(State {
                    slots,
                    retiring: Vec::new(),
                    pool: Pool::default(),
                }),
                submitted: Mutex::new([0; 2]),
                recorders: Mutex::new(Vec::new()),
                kernels: Mutex::new(FxHashMap::default()),
                completions,
                minted: AtomicU64::new(1),
            }),
            lane: Lane::Front,
        })
    }

    #[must_use]
    pub fn background(&self) -> Compute {
        Compute {
            shared: Arc::clone(&self.shared),
            lane: Lane::Back,
        }
    }

    pub fn image(&self, width: u32, height: u32, format: ImageFormat) -> Result<ComputeImage> {
        let mut images = self.images(&[[width, height]], format)?;
        Ok(images.remove(0))
    }

    pub fn images(&self, extents: &[[u32; 2]], format: ImageFormat) -> Result<Vec<ComputeImage>> {
        let tiles: Vec<ComputeTile<'_>> = extents.iter().copied().map(ComputeTile::New).collect();
        self.tiles(&tiles, format)
    }

    pub fn tiles(
        &self,
        tiles: &[ComputeTile<'_>],
        format: ImageFormat,
    ) -> Result<Vec<ComputeImage>> {
        if tiles.is_empty() {
            return Err(Error::Invalid("image list must not be empty".into()));
        }
        if !self.supports(format) {
            return Err(Error::Unsupported(format!(
                "device does not support {format:?} storage images"
            )));
        }
        ComputeImage::bind(&self.shared, tiles, format)
    }

    #[must_use]
    pub fn supports(&self, format: ImageFormat) -> bool {
        self.shared.supported.contains(&format)
    }

    #[must_use]
    pub fn workgroup_memory(&self) -> u32 {
        self.shared.device.limits().max_compute_shared_memory_size
    }

    #[must_use]
    pub fn widest_launch(&self) -> u32 {
        self.shared.device.limits().max_compute_work_group_count[0]
    }

    #[must_use]
    pub fn is_async(&self) -> bool {
        let device = &self.shared.device;
        Line::queue(device, self.lane).handle() != device.graphics_queue().handle()
    }

    pub fn buffer(&self, size: u64) -> Result<ComputeBuffer> {
        ComputeBuffer::device(&self.shared, size)
    }

    pub fn constants(&self, bytes: &[u8]) -> Result<ComputeBuffer> {
        ComputeBuffer::constants(&self.shared, bytes)
    }

    pub fn kernel(&self, shader: Shader) -> Result<Kernel> {
        Kernel::new(&self.shared, shader)
    }

    #[cfg(unix)]
    pub fn import_dmabuf(&self, spec: &DmabufSpec<'_>) -> Result<ForeignImage> {
        ForeignImage::import_dmabuf(&self.shared, spec)
    }

    pub fn video_target(&self, target: VideoTarget) -> Result<ForeignImage> {
        ForeignImage::video(&self.shared, target)
    }

    pub fn batch(&self) -> Batch {
        Batch::new(self.clone())
    }

    pub fn trim(&self) {
        self.shared.collect();
        let finished = self.shared.finished();
        for (lane, recorder) in self.shared.recorders.lock().iter_mut() {
            recorder.trim(&self.shared.device, finished[lane.index()]);
        }
    }

    pub fn wait_idle(&self) -> Result<()> {
        let submitted = *self.shared.submitted.lock();
        for lane in Lane::ALL {
            self.shared
                .line(lane)
                .timeline
                .wait(submitted[lane.index()])?;
        }
        self.shared.collect();
        Ok(())
    }
}

fn computable(device: &RenderDevice, format: ImageFormat) -> bool {
    let properties = unsafe {
        device
            .instance()
            .handle()
            .get_physical_device_format_properties(device.physical(), format.vk())
    };
    let required = vk::FormatFeatureFlags::STORAGE_IMAGE
        | vk::FormatFeatureFlags::SAMPLED_IMAGE
        | vk::FormatFeatureFlags::TRANSFER_SRC
        | vk::FormatFeatureFlags::TRANSFER_DST;
    properties.optimal_tiling_features.contains(required)
}

impl Shared {
    pub(crate) fn allocate_id(&self) -> u64 {
        self.minted.fetch_add(1, Ordering::Relaxed)
    }

    pub(crate) fn line(&self, lane: Lane) -> &Line {
        &self.lines[lane.index()]
    }

    pub(crate) fn finished(&self) -> Marks {
        [
            self.lines[0].timeline.completed(),
            self.lines[1].timeline.completed(),
        ]
    }

    pub(crate) fn retire(&self, after: Marks, resource: Box<dyn Any + Send>) {
        if reached(after, self.finished()) {
            drop(resource);
            return;
        }
        self.state.lock().retiring.push((after, resource));
    }

    pub(crate) fn collect(&self) {
        let finished = self.finished();
        let mut state = self.state.lock();
        let idle = state.pool.trim_idle();
        state.release_eviction(idle);
        let (done, waiting): (Vec<_>, Vec<_>) = std::mem::take(&mut state.retiring)
            .into_iter()
            .partition(|(after, _)| reached(*after, finished));
        state.retiring = waiting;
        state.slots.collect(finished);
        drop(state);
        drop(done);
    }

    pub(crate) fn recorder(&self, lane: Lane) -> Result<Recorder> {
        let mut recorders = self.recorders.lock();
        if let Some(at) = recorders.iter().position(|(held, _)| *held == lane) {
            return Ok(recorders.swap_remove(at).1);
        }
        drop(recorders);
        Recorder::new(&self.device, self.line(lane).family)
    }

    pub(crate) fn recycle(&self, lane: Lane, recorder: Recorder) {
        self.recorders.lock().push((lane, recorder));
    }
}

impl State {
    pub(crate) fn release_eviction(&mut self, evicted: Eviction) {
        for pooled in evicted.images {
            self.retiring.push((pooled.last, Box::new(pooled.image)));
        }
        for (last, buffer) in evicted.buffers {
            self.retiring.push((last, Box::new(buffer)));
        }
    }
}

impl Drop for Shared {
    fn drop(&mut self) {
        let submitted = *self.submitted.get_mut();
        for lane in Lane::ALL {
            if let Err(error) = self.lines[lane.index()]
                .timeline
                .wait(submitted[lane.index()])
            {
                tracing::error!("failed to wait for compute work before teardown: {error}");
            }
        }
        let state = self.state.get_mut();
        let evicted = state.pool.clear();
        state.release_eviction(evicted);
        drop(std::mem::take(&mut state.retiring));
        for (_, recorder) in std::mem::take(self.recorders.get_mut()) {
            recorder.destroy(&self.device);
        }
    }
}

impl std::fmt::Debug for Compute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Compute")
            .field("lane", &self.lane)
            .field("finished", &self.shared.finished())
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for Shared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shared").finish_non_exhaustive()
    }
}
