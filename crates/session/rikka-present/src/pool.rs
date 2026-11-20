use std::os::fd::{AsFd, OwnedFd};
use std::sync::Arc;
use std::task::Waker;
use std::time::{Duration, Instant};

use drm::control::syncobj;
use rikka_gpu::{DrmFormat, DrmNodeIds};
use rikka_renderer::{
    Acquired, DamageSource, DmabufFence, DmabufOutput, DmabufTarget, ExportedDmabuf, FrameStart,
    Recorded, Renderer, Scene, Sizing, Transform,
};
use rikka_types::Rect;
use rikka_wayland::{Display, EventLoop};
use rikka_wayland_client::wl_buffer::WlBuffer;
use rikka_wayland_client::wl_surface::WlSurface;
use rikka_wayland_client::wp_linux_drm_syncobj_manager_v1::WpLinuxDrmSyncobjManagerV1;
use rikka_wayland_client::wp_linux_drm_syncobj_surface_v1::WpLinuxDrmSyncobjSurfaceV1;
use rikka_wayland_client::wp_linux_drm_syncobj_timeline_v1::WpLinuxDrmSyncobjTimelineV1;
use rikka_wayland_client::zwp_linux_buffer_params_v1::Flags;
use rikka_wayland_client::zwp_linux_dmabuf_feedback_v1::ZwpLinuxDmabufFeedbackV1;
use rikka_wayland_client::zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1;

use crate::error::{Error, Result};
use crate::events::{Events, Signal, on_wl_buffer, on_zwp_linux_dmabuf_feedback_v1};
use crate::feedback::{Choice, Feedback};
use crate::sync::{RenderNode, readers_of, wait_signalled, written_by};

const XRGB8888: u32 = u32::from_le_bytes(*b"XR24");
const ARGB8888: u32 = u32::from_le_bytes(*b"AR24");
const XBGR8888: u32 = u32::from_le_bytes(*b"XB24");
const ABGR8888: u32 = u32::from_le_bytes(*b"AB24");

const SEE_THROUGH: [u32; 2] = [ARGB8888, ABGR8888];
const OPAQUE: [u32; 4] = [XRGB8888, XBGR8888, ARGB8888, ABGR8888];

#[derive(Debug)]
pub(crate) enum Acquisition {
    Reuse { id: u32, released: Option<OwnedFd> },
    Fresh,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Free,
    Drawing,
    Drawn,
    Attached,
}

#[derive(Debug)]
struct Timeline {
    handle: syncobj::Handle,
    proxy: WpLinuxDrmSyncobjTimelineV1,
    released_at: u64,
}

#[derive(Debug)]
struct Buffer {
    id: u32,
    target: DmabufTarget,
    wire: WlBuffer,
    size: (u32, u32),
    dmabuf: OwnedFd,
    timeline: Option<Timeline>,
    era: u64,
    state: State,
}

#[derive(Debug)]
struct Explicit {
    manager: WpLinuxDrmSyncobjManagerV1,
    surface: WpLinuxDrmSyncobjSurfaceV1,
    node: RenderNode,
}

pub(crate) struct Pool {
    output: DmabufOutput,
    buffers: Vec<Buffer>,
    next_id: u32,
    event_loop: EventLoop<Events>,
    events: Events,
    signal: Arc<Signal>,
    waker: Waker,
    factory: ZwpLinuxDmabufV1,
    feedback: ZwpLinuxDmabufFeedbackV1,
    formats: Option<Feedback>,
    choices: Vec<Choice>,
    exportable: Vec<DrmFormat>,
    nodes: Option<DrmNodeIds>,
    explicit: Option<Explicit>,
    limit: usize,
    transparent: bool,
    era: u64,
    presented_size: Option<(u32, u32)>,
    pub(crate) sizing: Sizing,
}

impl Pool {
    pub(crate) fn new(
        display: &Display,
        surface: &WlSurface,
        renderer: &mut Renderer,
        limit: usize,
        transparent: bool,
    ) -> Result<Pool> {
        let exportable = renderer.gpu().dmabuf_export_formats().to_vec();
        if exportable.is_empty() {
            return Err(Error::NoExport);
        }
        let mut event_loop = display.event_loop::<Events>();
        event_loop.on::<WlBuffer>(on_wl_buffer);
        event_loop.on::<ZwpLinuxDmabufFeedbackV1>(on_zwp_linux_dmabuf_feedback_v1);
        let factory: ZwpLinuxDmabufV1 = display.bind_on(&event_loop.route(), 4..=5)?;
        let feedback = factory.get_surface_feedback(surface);
        display.flush()?;

        let nodes = renderer.gpu().drm_nodes();
        let explicit = explicit_sync(display, surface, nodes);
        let signal = Arc::new(Signal::default());
        let mut pool = Pool {
            output: renderer.create_dmabuf_output(DamageSource::SinceLastFrame),
            buffers: Vec::new(),
            next_id: 0,
            event_loop,
            events: Events::default(),
            waker: Waker::from(Arc::clone(&signal)),
            signal,
            factory,
            feedback,
            formats: None,
            choices: Vec::new(),
            exportable,
            nodes,
            explicit,
            limit,
            transparent,
            era: 0,
            presented_size: None,
            sizing: Sizing::default(),
        };
        pool.hear()?;
        Ok(pool)
    }

    pub(crate) fn is_explicit(&self) -> bool {
        self.explicit.is_some()
    }

    pub(crate) fn set_transparent(&mut self, transparent: bool) {
        if self.transparent != transparent {
            self.transparent = transparent;
            self.choose();
        }
    }

    pub(crate) fn set_transform(&mut self, transform: Transform) {
        self.output.set_transform(transform);
    }

    pub(crate) fn start(
        &mut self,
        renderer: &mut Renderer,
        scene: &mut Scene,
    ) -> Result<FrameStart> {
        if self
            .presented_size
            .is_some_and(|size| !self.sizing.fits(size))
        {
            scene.damage_all();
        }
        let (width, height) = self.sizing.visible;
        Ok(renderer.start_dmabuf(scene, &self.output, width, height)?)
    }

    pub(crate) fn acquire(&mut self, timeout: Duration) -> Result<Acquired<Acquisition>> {
        let deadline = Instant::now() + timeout;
        loop {
            self.hear()?;
            self.reap()?;
            if self.formats.is_none() {
                if Instant::now() >= deadline {
                    return Ok(Acquired::NotReady);
                }
                self.signal.wait_until(deadline);
                continue;
            }
            if self.choices.is_empty() {
                return Err(Error::NoLayout);
            }
            if let Some(index) = self
                .buffers
                .iter()
                .position(|buffer| buffer.state == State::Free)
            {
                let buffer = &mut self.buffers[index];
                buffer.state = State::Drawing;
                let released = match self.explicit {
                    Some(_) => None,
                    None => readers_of(buffer.dmabuf.as_fd()).ok(),
                };
                return Ok(Acquired::Image(Acquisition::Reuse {
                    id: buffer.id,
                    released,
                }));
            }
            if self.buffers.len() < self.limit {
                tracing::debug!(
                    busy = ?self.buffers.iter().map(|buffer| buffer.state).collect::<Vec<_>>(),
                    "no buffer is free"
                );
                return Ok(Acquired::Image(Acquisition::Fresh));
            }
            if Instant::now() >= deadline {
                return Ok(Acquired::NotReady);
            }
            if let Some(released) = self.await_release(deadline)? {
                return Ok(Acquired::Image(released));
            }
        }
    }

    pub(crate) fn record(
        &mut self,
        renderer: &mut Renderer,
        scene: &mut Scene,
        acquisition: Acquisition,
    ) -> Result<Recorded> {
        let (index, released) = match acquisition {
            Acquisition::Reuse { id, released } => (self.index_of(id)?, released),
            Acquisition::Fresh => (self.allocate(renderer)?, None),
        };
        let (width, height) = self.sizing.visible;
        let buffer = &mut self.buffers[index];
        let mut onto = self
            .output
            .with_target(&mut buffer.target)
            .visible(width, height);
        if let Some(released) = released {
            onto = onto.with_release_fence(released);
        }
        let frame = match renderer.draw(scene, onto) {
            Ok(frame) => frame,
            Err(error) => {
                buffer.state = State::Free;
                return Err(error.into());
            }
        };
        if !frame.output.rendered() {
            buffer.state = State::Free;
            return Ok(match frame.output.needs_full_redraw {
                true => Recorded::deferred(),
                false => Recorded::default(),
            });
        }
        if let Err(error) = self.sign(index, frame.fence) {
            self.buffers[index].state = State::Free;
            return Err(error);
        }
        let buffer = &mut self.buffers[index];
        buffer.state = State::Drawn;
        Ok(Recorded::new(frame.output, buffer.id))
    }

    pub(crate) fn offer(&mut self, surface: &WlSurface, id: u32, damage: &[Rect]) -> Result<()> {
        let index = self.index_of(id)?;
        let (width, height) = self.sizing.visible;
        let buffer = &mut self.buffers[index];
        surface.attach(Some(&buffer.wire), 0, 0);
        let whole = Rect::new(0.0, 0.0, width as f32, height as f32);
        for rect in damage {
            let Some(rect) = rect.round_out().intersection(&whole) else {
                continue;
            };
            surface.damage_buffer(
                rect.min_x() as i32,
                rect.min_y() as i32,
                rect.width() as i32,
                rect.height() as i32,
            );
        }
        if let (Some(explicit), Some(timeline)) = (&self.explicit, &buffer.timeline) {
            let acquire = timeline.released_at - 1;
            let release = timeline.released_at;
            explicit.surface.set_acquire_point(
                &timeline.proxy,
                (acquire >> 32) as u32,
                acquire as u32,
            );
            explicit.surface.set_release_point(
                &timeline.proxy,
                (release >> 32) as u32,
                release as u32,
            );
        }
        buffer.state = State::Attached;
        self.presented_size = Some(buffer.size);
        Ok(())
    }

    pub(crate) fn withdraw(&mut self, id: u32) {
        if let Some(buffer) = self.buffers.iter_mut().find(|buffer| buffer.id == id) {
            buffer.state = State::Free;
        }
    }

    pub(crate) fn trim(&mut self) {
        if let Err(error) = self.hear().and_then(|()| self.reap()) {
            tracing::debug!("failed to identify released buffers: {error}");
        }
        let count = self.buffers.len();
        let mut index = 0;
        while index < self.buffers.len() {
            match self.buffers[index].state == State::Free {
                true => self.discard(index),
                false => index += 1,
            }
        }
        if self.buffers.len() < count {
            tracing::debug!(
                retained = self.buffers.len(),
                dropped = count - self.buffers.len(),
                "trimming idle buffers"
            );
        }
    }

    fn hear(&mut self) -> Result<()> {
        self.event_loop
            .poll_dispatch(&mut self.events, &self.waker)?;
        let released = std::mem::take(&mut self.events.released);
        if self.explicit.is_none() {
            for id in released {
                if let Some(buffer) = self.buffers.iter_mut().find(|buffer| buffer.id == id)
                    && buffer.state == State::Attached
                {
                    buffer.state = State::Free;
                }
            }
        }
        if let Some(formats) = self.events.feedback.take() {
            self.formats = Some(formats);
            self.choose();
        }
        Ok(())
    }

    fn reap(&mut self) -> Result<()> {
        if let Some(explicit) = &self.explicit {
            for buffer in &mut self.buffers {
                if buffer.state != State::Attached {
                    continue;
                }
                let Some(timeline) = &buffer.timeline else {
                    continue;
                };
                if explicit
                    .node
                    .is_signaled(timeline.handle, timeline.released_at)?
                {
                    buffer.state = State::Free;
                }
            }
        }
        let mut index = 0;
        while index < self.buffers.len() {
            let buffer = &self.buffers[index];
            let stale = buffer.era != self.era || !self.sizing.fits(buffer.size);
            match buffer.state == State::Free && stale {
                true => self.discard(index),
                false => index += 1,
            }
        }
        Ok(())
    }

    fn await_release(&mut self, deadline: Instant) -> Result<Option<Acquisition>> {
        let Some(explicit) = &self.explicit else {
            self.signal.wait_until(deadline);
            return Ok(None);
        };
        let busy: Vec<usize> = (0..self.buffers.len())
            .filter(|&index| self.buffers[index].state == State::Attached)
            .collect();
        let (timelines, points): (Vec<syncobj::Handle>, Vec<u64>) = busy
            .iter()
            .filter_map(|&index| self.buffers[index].timeline.as_ref())
            .map(|timeline| (timeline.handle, timeline.released_at))
            .unzip();
        if timelines.is_empty() {
            self.signal.wait_until(deadline);
            return Ok(None);
        }
        let Some(ready) = explicit.node.first_ready(&timelines, &points, deadline)? else {
            return Ok(None);
        };
        let index = busy[ready];
        let buffer = &self.buffers[index];
        let timeline = buffer
            .timeline
            .as_ref()
            .expect("buffers in flight should have timelines");
        if explicit
            .node
            .is_signaled(timeline.handle, timeline.released_at)?
            || !self.sizing.fits(buffer.size)
            || buffer.era != self.era
        {
            return Ok(None);
        }
        let released = explicit
            .node
            .fence_of(timeline.handle, timeline.released_at)?;
        let buffer = &mut self.buffers[index];
        buffer.state = State::Drawing;
        Ok(Some(Acquisition::Reuse {
            id: buffer.id,
            released: Some(released),
        }))
    }

    fn sign(&mut self, index: usize, fence: Option<DmabufFence>) -> Result<()> {
        let buffer = &mut self.buffers[index];
        match (&self.explicit, &mut buffer.timeline) {
            (Some(explicit), Some(timeline)) => {
                let acquire = timeline.released_at + 1;
                match fence {
                    Some(DmabufFence::SyncFile(fence)) => {
                        explicit
                            .node
                            .signal_after(timeline.handle, acquire, fence.as_fd())?;
                    }
                    Some(DmabufFence::Readable(fence)) => {
                        wait_signalled(fence.as_fd())?;
                        explicit.node.signal(timeline.handle, acquire)?;
                    }
                    None => explicit.node.signal(timeline.handle, acquire)?,
                }
                timeline.released_at = acquire + 1;
            }
            _ => match fence {
                Some(DmabufFence::SyncFile(fence)) => {
                    if written_by(buffer.dmabuf.as_fd(), fence.as_fd()).is_err() {
                        wait_signalled(fence.as_fd())?;
                    }
                }
                Some(DmabufFence::Readable(fence)) => wait_signalled(fence.as_fd())?,
                None => {}
            },
        }
        Ok(())
    }

    fn allocate(&mut self, renderer: &mut Renderer) -> Result<usize> {
        let (width, height) = self.sizing.allocation();
        let mut refused = None;
        for choice in &self.choices {
            match renderer.export_dmabuf(width, height, choice.fourcc, &choice.modifiers) {
                Ok(exported) => return self.adopt(exported, (width, height)),
                Err(error) => {
                    tracing::debug!(
                        fourcc = choice.fourcc,
                        scanout = choice.scanout,
                        "failed to export buffer: {error}"
                    );
                    refused = Some(error);
                }
            }
        }
        Err(match refused {
            Some(error) => error.into(),
            None => Error::NoLayout,
        })
    }

    fn adopt(&mut self, exported: ExportedDmabuf, size: (u32, u32)) -> Result<usize> {
        let ExportedDmabuf {
            target,
            format,
            planes,
        } = exported;
        let dmabuf = planes.first().ok_or(Error::NoLayout)?.fd.try_clone()?;
        tracing::debug!(
            width = size.0,
            height = size.1,
            fourcc = format.fourcc,
            modifier = format.modifier,
            buffers = self.buffers.len(),
            "allocated buffer"
        );
        let timeline = match &self.explicit {
            Some(explicit) => {
                let (handle, fd) = explicit.node.timeline()?;
                let proxy = explicit.manager.import_timeline(fd.as_fd());
                Some(Timeline {
                    handle,
                    proxy,
                    released_at: 0,
                })
            }
            None => None,
        };
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        let params = self.factory.create_params();
        for (plane, layout) in planes.iter().enumerate() {
            params.add(
                layout.fd.as_fd(),
                plane as u32,
                layout.offset,
                layout.stride,
                (format.modifier >> 32) as u32,
                format.modifier as u32,
            );
        }
        let wire = params
            .create_immed(size.0 as i32, size.1 as i32, format.fourcc, Flags::empty())
            .with_key(id);
        params.destroy();
        self.buffers.push(Buffer {
            id,
            target,
            wire,
            size,
            dmabuf,
            timeline,
            era: self.era,
            state: State::Drawing,
        });
        Ok(self.buffers.len() - 1)
    }

    fn discard(&mut self, index: usize) {
        let buffer = self.buffers.swap_remove(index);
        buffer.wire.destroy();
        if let Some(timeline) = buffer.timeline {
            timeline.proxy.destroy();
            if let Some(explicit) = &self.explicit {
                explicit.node.forget(timeline.handle);
            }
        }
    }

    fn index_of(&self, id: u32) -> Result<usize> {
        self.buffers
            .iter()
            .position(|buffer| buffer.id == id)
            .ok_or(Error::Gone)
    }

    fn choose(&mut self) {
        let Some(formats) = &self.formats else {
            return;
        };
        let local = match (formats.main, self.nodes) {
            (Some(main), Some(nodes)) => nodes.matches((libc::major(main), libc::minor(main))),
            _ => true,
        };
        let fourccs: &[u32] = match self.transparent {
            true => &SEE_THROUGH,
            false => &OPAQUE,
        };
        let choices = formats.choices(fourccs, &self.exportable, local);
        if choices.first() != self.choices.first() {
            self.era += 1;
        }
        self.choices = choices;
    }
}

fn explicit_sync(
    display: &Display,
    surface: &WlSurface,
    nodes: Option<DrmNodeIds>,
) -> Option<Explicit> {
    let manager = display.bind_optional::<WpLinuxDrmSyncobjManagerV1>(1..=1)?;
    let (major, minor) = nodes.and_then(|nodes| nodes.render.or(nodes.primary))?;
    let node = match RenderNode::open(major, minor) {
        Ok(node) => node,
        Err(error) => {
            tracing::info!("explicit sync unavailable: {error}");
            manager.destroy();
            return None;
        }
    };
    let synced = manager.get_surface(surface);
    Some(Explicit {
        manager,
        surface: synced,
        node,
    })
}

impl Drop for Pool {
    fn drop(&mut self) {
        while !self.buffers.is_empty() {
            self.discard(self.buffers.len() - 1);
        }
        self.feedback.destroy();
        if self.factory.version() >= 3 {
            self.factory.destroy();
        }
        if let Some(explicit) = self.explicit.take() {
            explicit.surface.destroy();
            explicit.manager.destroy();
        }
    }
}

impl std::fmt::Debug for Pool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pool")
            .field("buffers", &self.buffers.len())
            .field("explicit", &self.explicit.is_some())
            .field("sizing", &self.sizing)
            .finish_non_exhaustive()
    }
}
