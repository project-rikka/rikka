use std::collections::BTreeMap;
use std::fmt;
use std::io;
use std::num::NonZeroU32;
use std::os::fd::{AsFd, BorrowedFd};

use drm::control::{AtomicCommitFlags, Device as _};
use rikka_compositor_reactor::Sender;
use rikka_compositor_types::{FormatSet, Fourcc, Modifier};

use crate::allocator::Allocator;
use crate::atomic::AtomicRequest;
use crate::blob::Blob;
use crate::buffers::Buffers;
use crate::claim::{Claimed, claim_planes, pick_crtc};
use crate::committer::{Command, Committer, CursorControl, Hardware, Setup};
use crate::connector::Connector;
use crate::cursor::CursorPool;
use crate::dmabuf::Dmabuf;
use crate::drm_fd::DrmFd;
use crate::error::{Error, Result};
use crate::event::KmsEvent;
use crate::frame_commit::{FrameCommit, Modeset, PlaneScanout, PlaneUpdate};
use crate::framebuffer::{Framebuffer, Origin};
use crate::gpu::Gpu;
use crate::hdr::Signal;
use crate::ids::CrtcId;
use crate::import::import;
use crate::lease::Reserved;
use crate::negotiate::{self, Candidate, candidates, linear_candidates, linear_fourccs};
use crate::output::Output;
use crate::output_config::OutputConfig;
use crate::present::Present;
use crate::properties::{ConnectorProps, ObjectProperties, Signalling};
use crate::rect::Rect;
use crate::resources::Resources;
use crate::session::Session;
use crate::swapchain::{Buffer, Memory, Swapchain};
use crate::timing::{Timing, VblankTimeline};

const SWAPCHAIN_LENGTH: usize = 3;
const HDR_BITS: u32 = 10;

pub struct Device {
    gpu: Gpu,
    pub(crate) fd: DrmFd,
    pub(crate) resources: Resources,
    allocator: Allocator,
    committer: Committer,
    cursors: BTreeMap<CrtcId, CursorPool>,
    active_crtcs: BTreeMap<CrtcId, Claimed>,
    pub(crate) leased: BTreeMap<NonZeroU32, Reserved>,
}

struct CrtcAssignment {
    crtc_index: usize,
    planes: Claimed,
}

struct Attempt<'a> {
    connector: &'a Connector,
    assignment: &'a CrtcAssignment,
    lighting: &'a OutputConfig,
    blob: &'a Blob,
    metadata: Option<&'a Blob>,
}

impl Attempt<'_> {
    fn signalling(&self) -> Signalling {
        match self.metadata {
            Some(blob) => Signalling::Pq {
                metadata: blob.id(),
            },
            None => Signalling::Sdr,
        }
    }
}

#[derive(Clone, Copy)]
enum Allocation<'a> {
    Gbm(Option<&'a FormatSet>),
    Dumb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Probe {
    Forced,
    Cached,
}

impl Device {
    pub fn open(session: &Session, gpu: &Gpu, events: Sender<KmsEvent>) -> Result<Self> {
        let fd = DrmFd::new(session.open_device(gpu.path())?);
        match Self::start(&fd, gpu, events) {
            Ok(device) => Ok(device),
            Err(error) => {
                if let Ok(owned) = fd.into_owned()
                    && let Err(closing) = session.close_device(owned)
                {
                    tracing::warn!(error = %closing, "failed to close device");
                }
                Err(error)
            }
        }
    }

    fn start(fd: &DrmFd, gpu: &Gpu, events: Sender<KmsEvent>) -> Result<Self> {
        let resources = Resources::read(fd)?;
        let allocator = Allocator::new(fd);
        let connectors = connector_props(fd, Probe::Forced)?;
        tracing::info!(
            gpu = %gpu.path().display(),
            crtcs = resources.crtcs.len(),
            planes = resources.planes.len(),
            connectors = connectors.len(),
            "GPU opened for display"
        );
        let hardware = Hardware {
            caps: resources.caps,
            crtcs: resources.crtc_props.clone(),
            planes: resources.planes.iter().map(|plane| plane.props).collect(),
            connectors,
        };
        let committer = Committer::spawn(gpu.minor() as usize, fd.clone(), hardware, events)?;
        Ok(Self {
            gpu: gpu.clone(),
            fd: fd.clone(),
            resources,
            allocator,
            committer,
            cursors: BTreeMap::new(),
            active_crtcs: BTreeMap::new(),
            leased: BTreeMap::new(),
        })
    }

    pub fn gpu(&self) -> &Gpu {
        &self.gpu
    }

    pub fn connectors(&self) -> Result<Vec<Connector>> {
        let handles = self.fd.resource_handles().map_err(Error::Query)?;
        Ok(handles
            .connectors()
            .iter()
            .filter_map(|handle| Connector::read(&self.fd, *handle, &handles))
            .collect())
    }

    pub fn primary_formats(&self, connector: &Connector) -> Result<FormatSet> {
        let assignment = self.choose(connector)?;
        Ok(self.resources.planes[assignment.planes.primary]
            .formats
            .clone())
    }

    pub fn enable_output(
        &mut self,
        connector: &Connector,
        lighting: OutputConfig,
        buffers: Buffers,
    ) -> Result<Output> {
        let assignment = self.choose(connector)?;
        let crtc = CrtcId::from_handle(self.resources.crtcs[assignment.crtc_index]);
        let lighting = self.fit(connector, crtc, lighting)?;
        let blob = Blob::mode(&self.fd, &lighting.mode).map_err(Error::Blob)?;
        let metadata = match &lighting.signal {
            Signal::Sdr => None,
            Signal::Pq(metadata) => {
                Some(Blob::hdr_metadata(&self.fd, metadata).map_err(Error::Blob)?)
            }
        };
        let plane = &self.resources.planes[assignment.planes.primary].formats;
        let explicit = self.resources.caps.explicit_modifiers;
        let preferred = negotiate::preferred(&lighting.signal);
        let attempt = Attempt {
            connector,
            assignment: &assignment,
            lighting: &lighting,
            blob: &blob,
            metadata: metadata.as_ref(),
        };
        let deep = |fourcc: &Fourcc| attempt.metadata.is_none() || preferred.contains(fourcc);
        let swapchain = match buffers {
            Buffers::Imported(dmabufs)
                if !dmabufs.iter().all(|dmabuf| deep(&dmabuf.format().fourcc)) =>
            {
                return Err(Error::HdrUnsupported(connector.name().to_owned()));
            }
            Buffers::Mapped(fourccs) if !fourccs.iter().any(deep) => {
                return Err(Error::HdrUnsupported(connector.name().to_owned()));
            }
            Buffers::Imported(dmabufs) => self.imported(&attempt, dmabufs)?,
            Buffers::Rendered(renderable) => self.allocated(
                &attempt,
                candidates(plane, &renderable, explicit, preferred),
                Allocation::Gbm(Some(&renderable)),
            )?,
            Buffers::Shared(renderable) => self.allocated(
                &attempt,
                linear_candidates(plane, &renderable, explicit, preferred),
                Allocation::Gbm(None),
            )?,
            Buffers::Mapped(fourccs) => self.allocated(
                &attempt,
                linear_fourccs(plane, &fourccs)
                    .filter(deep)
                    .map(|fourcc| Candidate {
                        fourcc,
                        modifiers: vec![Modifier::INVALID],
                        linear: true,
                    })
                    .collect(),
                Allocation::Dumb,
            )?,
        };
        self.enable(
            connector,
            crtc,
            assignment,
            lighting,
            (blob, metadata),
            swapchain,
        )
    }

    pub fn reconfigure_output(
        &mut self,
        output: &mut Output,
        connector: &Connector,
        lighting: OutputConfig,
        buffers: Buffers,
    ) -> Result<()> {
        if output.connector != connector.id() {
            return Err(Error::UnknownOutput(connector.name().to_owned()));
        }
        if output.mode == lighting.mode
            && output.max_bpc == lighting.max_bpc
            && output.signal == lighting.signal
        {
            return self.set_vrr(output, lighting.vrr);
        }
        let planes = self
            .active_crtcs
            .remove(&output.crtc)
            .ok_or_else(|| Error::UnknownOutput(output.name.clone()))?;
        match self.enable_output(connector, lighting, buffers) {
            Ok(reconfigured) => {
                if reconfigured.crtc != output.crtc {
                    self.committer.send(Command::Disable(output.crtc))?;
                }
                *output = reconfigured;
                Ok(())
            }
            Err(error) => {
                self.active_crtcs.insert(output.crtc, planes);
                Err(error)
            }
        }
    }

    pub fn disable_output(&mut self, output: Output) -> Result<()> {
        self.active_crtcs.remove(&output.crtc);
        tracing::info!("{} disabled", output.name);
        self.committer.send(Command::Disable(output.crtc))
    }

    pub fn set_vrr(&self, output: &mut Output, enabled: bool) -> Result<()> {
        let enabled = enabled && output.vrr_capable;
        if output.vrr == enabled {
            return Ok(());
        }
        output.vrr = enabled;
        self.committer.send(Command::Vrr {
            crtc: output.crtc,
            enabled,
        })
    }

    pub fn present(&self, present: Present) -> Result<()> {
        self.committer.send(Command::Present(present))
    }

    pub fn cursor_size(&self) -> (u32, u32) {
        self.resources.caps.cursor
    }

    pub fn set_cursor_pixels(
        &mut self,
        crtc: CrtcId,
        width: u32,
        height: u32,
        argb: &[u8],
        hotspot: (i32, i32),
    ) -> Result<()> {
        let plane = self.resources.caps.cursor;
        let image = self
            .cursors
            .entry(crtc)
            .or_insert_with(|| CursorPool::new(plane))
            .draw(&self.allocator, width, height, argb, hotspot)?;
        self.committer.send(Command::Cursor {
            crtc,
            image: Some(image),
        })
    }

    pub fn clear_cursor(&self, crtc: CrtcId) -> Result<()> {
        self.committer.send(Command::Cursor { crtc, image: None })
    }

    pub fn import(&self, dmabuf: &Dmabuf) -> Result<Framebuffer> {
        import(
            &self.fd,
            self.resources.caps.explicit_modifiers,
            dmabuf,
            Origin::Client,
        )
    }

    pub fn connector_names(&self) -> Result<Vec<String>> {
        let handles = self.fd.resource_handles().map_err(Error::Query)?;
        Ok(handles
            .connectors()
            .iter()
            .filter_map(|handle| Connector::name_of(&self.fd, *handle))
            .collect())
    }

    pub fn cursor_control(&self) -> CursorControl {
        self.committer.cursor_control()
    }

    pub fn pause(&self) -> Result<()> {
        self.committer.send(Command::Pause)
    }

    pub fn resume(&self) -> Result<()> {
        let connectors = connector_props(&self.fd, Probe::Cached)?;
        self.committer.send(Command::Resume(connectors))
    }

    pub fn close(self, session: &Session) -> Result<()> {
        let Self {
            gpu,
            fd,
            allocator,
            committer,
            cursors,
            ..
        } = self;
        drop(committer);
        drop(cursors);
        drop(allocator);
        match fd.into_owned() {
            Ok(owned) => session.close_device(owned),
            Err(_) => {
                tracing::warn!(
                    "{} stays open until its last framebuffer is dropped",
                    gpu.path().display()
                );
                Ok(())
            }
        }
    }

    fn choose(&self, connector: &Connector) -> Result<CrtcAssignment> {
        let reachable = self.resources.crtc_mask(&connector.reachable_crtcs);
        let current = connector
            .current_crtc
            .and_then(|crtc| self.resources.crtc_index(crtc));
        let crtc_index = pick_crtc(current, reachable, self.taken_crtcs())
            .ok_or_else(|| Error::NoCrtc(connector.name().to_owned()))?;
        let planes = claim_planes(
            &self.resources.plane_facts(),
            crtc_index,
            &self.claimed_planes(),
        )
        .ok_or_else(|| Error::NoPrimaryPlane(connector.name().to_owned()))?;
        Ok(CrtcAssignment { crtc_index, planes })
    }

    pub(crate) fn taken_crtcs(&self) -> u32 {
        self.active_crtcs
            .keys()
            .filter_map(|crtc| self.resources.crtc_index(crtc.handle()))
            .chain(
                self.leased
                    .values()
                    .flat_map(|reserved| reserved.crtcs.iter().copied()),
            )
            .fold(0u32, |mask, index| mask | (1 << index))
    }

    pub(crate) fn claimed_planes(&self) -> Vec<bool> {
        let mut claimed = vec![false; self.resources.planes.len()];
        let active_crtcs = self
            .active_crtcs
            .values()
            .flat_map(|active_crtcs| {
                [
                    Some(active_crtcs.primary),
                    active_crtcs.cursor,
                    active_crtcs.overlay,
                ]
            })
            .flatten();
        let leased = self
            .leased
            .values()
            .flat_map(|reserved| reserved.planes.iter().copied());
        for index in active_crtcs.chain(leased) {
            claimed[index] = true;
        }
        claimed
    }

    fn fit(
        &self,
        connector: &Connector,
        crtc: CrtcId,
        lighting: OutputConfig,
    ) -> Result<OutputConfig> {
        let vrr_property = self
            .resources
            .crtc_index(crtc.handle())
            .is_some_and(|index| self.resources.crtc_props[index].vrr_enabled.is_some());
        let hdr = matches!(lighting.signal, Signal::Pq(_));
        if hdr && !connector.signals_pq() {
            return Err(Error::HdrUnsupported(connector.name().to_owned()));
        }
        let wanted = match hdr {
            true => Some(lighting.max_bpc.unwrap_or(HDR_BITS).max(HDR_BITS)),
            false => lighting.max_bpc,
        };
        let max_bpc = match (wanted, connector.max_bpc_range()) {
            (Some(bits), Some((low, high))) => Some(u64::from(bits).clamp(low, high) as u32),
            _ => None,
        };
        Ok(OutputConfig {
            mode: lighting.mode,
            vrr: lighting.vrr && connector.vrr_capable() && vrr_property,
            max_bpc,
            signal: lighting.signal,
        })
    }

    fn allocated(
        &self,
        attempt: &Attempt<'_>,
        offered: Vec<Candidate>,
        allocation: Allocation<'_>,
    ) -> Result<Swapchain> {
        let mut refusal = None;
        for candidate in &offered {
            let built = self
                .first_buffer(candidate, attempt.lighting, allocation)
                .and_then(|first| {
                    self.test_modeset(attempt, &first.framebuffer)?;
                    let format = first.framebuffer.format();
                    let mut allocated = vec![first];
                    for _ in 1..SWAPCHAIN_LENGTH {
                        allocated.push(self.allocate(candidate, attempt.lighting, allocation)?);
                    }
                    Ok(Swapchain::new(allocated, format))
                });
            match built {
                Ok(swapchain) => return Ok(swapchain),
                Err(error) => {
                    tracing::debug!(
                        "{} rejected {:?} with {} modifiers: {error}",
                        attempt.connector.name(),
                        candidate.fourcc,
                        candidate.modifiers.len()
                    );
                    refusal = Some(error);
                }
            }
        }
        Err(refusal.unwrap_or(Error::NoScanoutFormat))
    }

    fn first_buffer(
        &self,
        candidate: &Candidate,
        lighting: &OutputConfig,
        allocation: Allocation<'_>,
    ) -> Result<Buffer> {
        let first = self.allocate(candidate, lighting, allocation)?;
        if let (Allocation::Gbm(Some(renderable)), Memory::Dmabuf(dmabuf)) =
            (allocation, &first.memory)
            && !candidate.accepts(renderable, dmabuf.format().modifier)
        {
            return Err(Error::NoScanoutFormat);
        }
        Ok(first)
    }

    fn allocate(
        &self,
        candidate: &Candidate,
        lighting: &OutputConfig,
        allocation: Allocation<'_>,
    ) -> Result<Buffer> {
        let (width, height) = (lighting.mode.width(), lighting.mode.height());
        Ok(match allocation {
            Allocation::Dumb => {
                let (framebuffer, mapped) =
                    self.allocator
                        .mapped(width, height, candidate.fourcc, Origin::Swapchain)?;
                Buffer {
                    framebuffer,
                    memory: Memory::Mapped(mapped),
                }
            }
            Allocation::Gbm(_) => {
                let (framebuffer, dmabuf) = self.allocator.scanout(width, height, candidate)?;
                Buffer {
                    framebuffer,
                    memory: Memory::Dmabuf(dmabuf),
                }
            }
        })
    }

    fn imported(&self, attempt: &Attempt<'_>, dmabufs: Vec<Dmabuf>) -> Result<Swapchain> {
        let mode = attempt.lighting.mode;
        let Some(format) = dmabufs.first().map(Dmabuf::format) else {
            return Err(Error::NoScanoutFormat);
        };
        let mut allocated = Vec::with_capacity(dmabufs.len());
        for dmabuf in dmabufs {
            if (dmabuf.width(), dmabuf.height()) != (mode.width(), mode.height())
                || dmabuf.format() != format
            {
                return Err(Error::Import(io::Error::from(io::ErrorKind::InvalidInput)));
            }
            let framebuffer = import(
                &self.fd,
                self.resources.caps.explicit_modifiers,
                &dmabuf,
                Origin::Swapchain,
            )?;
            allocated.push(Buffer {
                framebuffer,
                memory: Memory::Dmabuf(dmabuf),
            });
        }
        self.test_modeset(attempt, &allocated[0].framebuffer)?;
        Ok(Swapchain::new(allocated, format))
    }

    fn test_modeset(&self, attempt: &Attempt<'_>, framebuffer: &Framebuffer) -> Result<()> {
        let mut request = AtomicRequest::default();
        self.initial_commit(attempt, framebuffer)
            .encode(&mut request);
        request
            .submit(
                &self.fd,
                AtomicCommitFlags::TEST_ONLY | AtomicCommitFlags::ALLOW_MODESET,
            )
            .map_err(|source| Error::ConfigurationRejected {
                connector: attempt.connector.name().to_owned(),
                source,
            })
    }

    fn initial_commit(&self, attempt: &Attempt<'_>, framebuffer: &Framebuffer) -> FrameCommit {
        let whole = Rect::sized(framebuffer.width(), framebuffer.height());
        FrameCommit {
            modeset: Some(Modeset {
                connector: attempt.connector.props,
                mode_blob: attempt.blob.id(),
                max_bpc: attempt.lighting.max_bpc.map(u64::from),
                signalling: attempt.signalling(),
            }),
            vrr: attempt.lighting.vrr.then_some(true),
            primary: Some(PlaneUpdate::Show(PlaneScanout {
                plane: self.resources.planes[attempt.assignment.planes.primary].props,
                framebuffer: framebuffer.id().raw(),
                format: framebuffer.format(),
                source: whole,
                destination: whole,
                fence: None,
                damage: None,
            })),
            ..FrameCommit::new(self.resources.crtc_props[attempt.assignment.crtc_index])
        }
    }

    fn enable(
        &mut self,
        connector: &Connector,
        crtc: CrtcId,
        assignment: CrtcAssignment,
        lighting: OutputConfig,
        (blob, metadata): (Blob, Option<Blob>),
        swapchain: Swapchain,
    ) -> Result<Output> {
        let props = |index: usize| self.resources.planes[index].props;
        let timeline = VblankTimeline::new(Timing {
            refresh: lighting.mode.refresh(),
            vrr: lighting.vrr,
            last_vblank: None,
            sequence: 0,
            lead: lighting.mode.vblank(),
        });
        let setup = Setup {
            crtc,
            crtc_props: self.resources.crtc_props[assignment.crtc_index],
            connector: connector.props,
            mode: lighting.mode,
            blob,
            metadata,
            primary: props(assignment.planes.primary),
            cursor: assignment.planes.cursor.map(props),
            overlay: assignment.planes.overlay.map(props),
            vrr: lighting.vrr,
            max_bpc: lighting.max_bpc.map(u64::from),
            timeline: timeline.clone(),
        };
        self.committer.send(Command::Enable(Box::new(setup)))?;
        self.active_crtcs.insert(crtc, assignment.planes);
        let mode = lighting.mode;
        tracing::info!(
            "{} enabled at {}x{}@{:.2}Hz on {crtc}{}",
            connector.name(),
            mode.width(),
            mode.height(),
            f64::from(mode.refresh_millihertz()) / 1000.0,
            match lighting.signal {
                Signal::Sdr => "",
                Signal::Pq(_) => " in HDR",
            }
        );
        Ok(Output {
            crtc,
            connector: connector.id(),
            name: connector.name().to_owned(),
            mode,
            vrr: lighting.vrr,
            vrr_capable: connector.vrr_capable(),
            max_bpc: lighting.max_bpc,
            signal: lighting.signal,
            swapchain,
            timeline,
            scanout_formats: self.resources.planes[assignment.planes.primary]
                .formats
                .clone(),
            cursor_plane: assignment.planes.cursor.is_some(),
            async_flips: self.resources.caps.async_flips,
        })
    }
}

fn connector_props(fd: &DrmFd, probe: Probe) -> Result<Vec<ConnectorProps>> {
    let handles = fd.resource_handles().map_err(Error::Query)?;
    Ok(handles
        .connectors()
        .iter()
        .filter_map(|handle| {
            if probe == Probe::Forced
                && let Err(error) = fd.get_connector(*handle, true)
            {
                tracing::debug!("failed to probe connector {}: {error}", u32::from(*handle));
            }
            let properties = ObjectProperties::read(fd, *handle).ok()?;
            ConnectorProps::resolve(&properties).ok()
        })
        .collect())
}

impl AsFd for Device {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

impl fmt::Debug for Device {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Device")
            .field("gpu", &self.gpu)
            .field(
                "active_crtcs",
                &self.active_crtcs.keys().collect::<Vec<_>>(),
            )
            .field("committer", &self.committer)
            .finish_non_exhaustive()
    }
}
