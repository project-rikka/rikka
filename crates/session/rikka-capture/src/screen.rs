use std::collections::HashMap;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gbm::BufferObjectFlags;
use rikka_wayland::{Display, EventLoop};
use rikka_wayland_client::ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1;
use rikka_wayland_client::ext_foreign_toplevel_image_capture_source_manager_v1::ExtForeignToplevelImageCaptureSourceManagerV1;
use rikka_wayland_client::ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1;
use rikka_wayland_client::ext_image_capture_source_v1::ExtImageCaptureSourceV1;
use rikka_wayland_client::ext_image_copy_capture_manager_v1::{
    ExtImageCopyCaptureManagerV1, Options,
};
use rikka_wayland_client::ext_output_image_capture_source_manager_v1::ExtOutputImageCaptureSourceManagerV1;
use rikka_wayland_client::zwp_linux_buffer_params_v1::Flags;
use rikka_wayland_client::zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1;
use rikka_wayland_client::{wl_output, wl_pointer, wl_seat, wl_shm};
use rikka_worker::wait_readable;
use rustix::fs::{MemfdFlags, SealFlags, fcntl_add_seals, ftruncate, memfd_create};

use crate::buffer::{CaptureBuffer, Mapping, Memory, OwnedPlane};
use crate::constraints::Constraints;
use crate::error::{Error, Result};
use crate::format::PixelFormat;
use crate::frames::{CursorFrames, CursorSlot, Frames, Slot, cursor_sessions, sessions};
use crate::source::{ScreenOutput, ScreenWindow, Source};
use crate::state::{
    State, on_ext_foreign_toplevel_handle_v1, on_ext_foreign_toplevel_list_v1, on_wl_output,
};

const MODIFIER_INVALID: u64 = 0x00ff_ffff_ffff_ffff;

const DRM_NODES: &str = "/dev/dri";

const MEMFD_NAME: &str = "rikka-capture";

const NEGOTIATION_TIMEOUT: Duration = Duration::from_secs(5);

pub struct Screen {
    event_loop: EventLoop<State>,
    state: State,
    manager: ExtImageCopyCaptureManagerV1,
    output_source: Option<ExtOutputImageCaptureSourceManagerV1>,
    window_source: Option<ExtForeignToplevelImageCaptureSourceManagerV1>,
    seat: Option<wl_seat::WlSeat>,
    pointer: Option<wl_pointer::WlPointer>,
    devices: HashMap<u64, Arc<gbm::Device<OwnedFd>>>,
}

impl Screen {
    pub fn connect() -> Result<Self> {
        Self::new(Display::from_env()?)
    }

    pub fn connect_to(socket: &Path) -> Result<Self> {
        Self::new(Display::connect_to(socket)?)
    }

    fn new(display: Display) -> Result<Self> {
        let mut event_loop = display.event_loop::<State>();
        event_loop.on::<wl_output::WlOutput>(on_wl_output);
        event_loop.on::<ExtForeignToplevelListV1>(on_ext_foreign_toplevel_list_v1);
        event_loop.on::<ExtForeignToplevelHandleV1>(on_ext_foreign_toplevel_handle_v1);
        let route = event_loop.route();

        let shm: wl_shm::WlShm = display.bind(1..=1)?;
        let manager: ExtImageCopyCaptureManagerV1 = display.bind(1..=1)?;
        let output_source = display.bind(1..=1).ok();
        let window_source = display.bind(1..=1).ok();
        let seat = display.bind(1..=5).ok();
        let dmabuf: Option<ZwpLinuxDmabufV1> = display.bind(2..=5).ok();
        let _: Result<ExtForeignToplevelListV1, _> = display.bind_on(&route, 1..=1);

        let mut state = State::new(shm, dmabuf);
        for output in display.bind_all::<wl_output::WlOutput>(&route, 4) {
            state.add_output(output);
        }
        event_loop.roundtrip(&mut state)?;

        Ok(Self {
            event_loop,
            state,
            manager,
            output_source,
            window_source,
            seat,
            pointer: None,
            devices: HashMap::new(),
        })
    }

    #[must_use]
    pub fn outputs(&self) -> Vec<ScreenOutput> {
        self.state
            .outputs
            .iter()
            .filter(|output| !output.info.name.is_empty())
            .map(|output| output.info.clone())
            .collect()
    }

    #[must_use]
    pub fn windows(&self) -> Vec<ScreenWindow> {
        self.state
            .windows
            .iter()
            .filter(|window| !window.info.identifier.is_empty())
            .map(|window| window.info.clone())
            .collect()
    }

    #[must_use]
    pub fn changes(&self) -> u64 {
        self.state.changes
    }

    pub fn frames(&mut self, source: &Source, cursor: bool) -> Result<Frames> {
        let source = self.source_object(source)?;
        let options = match cursor {
            true => Options::PAINT_CURSORS,
            false => Options::empty(),
        };
        let slot = Arc::new(Slot::default());
        let session = self
            .manager
            .via(&sessions(&slot))
            .create_session(&source, options);
        Ok(Frames::new(session, Some(source), slot))
    }

    pub fn cursor_frames(&mut self, source: &Source) -> Result<CursorFrames> {
        let pointer = self.pointer()?;
        let source = self.source_object(source)?;
        let cursor_slot = Arc::new(CursorSlot::default());
        let cursor = self
            .manager
            .via(&cursor_sessions(&cursor_slot))
            .create_pointer_cursor_session(&source, &pointer);
        let slot = Arc::new(Slot::default());
        let session = cursor.via(&sessions(&slot)).get_capture_session();
        let frames = Frames::new(session, None, slot);
        Ok(CursorFrames::new(cursor, source, cursor_slot, frames))
    }

    pub fn negotiate(&mut self, frames: &mut Frames) -> Result<Constraints> {
        let deadline = Instant::now() + NEGOTIATION_TIMEOUT;
        while frames.generation() == 0 && !frames.is_stopped() {
            if !self.dispatch(Some(deadline))? {
                return Err(Error::Failed(
                    "no buffer constraints received from capture session",
                ));
            }
        }
        if frames.is_stopped() {
            return Err(Error::Stopped);
        }
        frames.events();
        Ok(frames.constraints())
    }

    pub fn shm_buffer(
        &mut self,
        format: PixelFormat,
        width: u32,
        height: u32,
    ) -> Result<CaptureBuffer> {
        if width == 0 || height == 0 {
            return Err(Error::Failed("requested buffer has zero size"));
        }
        let stride = width * 4;
        let len = stride as usize * height as usize;
        let fd = memfd(len)?;
        let map = Mapping::map(fd.as_fd(), len)?;

        let pool = self.state.shm.create_pool(fd.as_fd(), len as i32);
        let wl = pool.create_buffer(
            0,
            width as i32,
            height as i32,
            stride as i32,
            format.to_shm(),
        );
        Ok(CaptureBuffer::new(
            wl,
            Memory::Shared { fd, map, pool },
            format,
            width,
            height,
            stride,
        ))
    }

    pub fn probe_modifier(
        &mut self,
        device: u64,
        format: PixelFormat,
        modifiers: &[u64],
        width: u32,
        height: u32,
    ) -> Result<u64> {
        let (_, modifier) = self.allocate(device, format, modifiers, width, height)?;
        Ok(modifier)
    }

    fn allocate(
        &mut self,
        device: u64,
        format: PixelFormat,
        modifiers: &[u64],
        width: u32,
        height: u32,
    ) -> Result<(gbm::BufferObject<()>, u64)> {
        let code = gbm::Format::try_from(format.fourcc())
            .map_err(|_| Error::Failed("unsupported DRM fourcc format"))?;
        let gbm = self.device(device)?;

        let named: Vec<gbm::Modifier> = modifiers
            .iter()
            .copied()
            .filter(|modifier| *modifier != MODIFIER_INVALID)
            .map(gbm::Modifier::from)
            .collect();
        let bo: gbm::BufferObject<()> = match named.is_empty() {
            true => gbm
                .create_buffer_object(width, height, code, BufferObjectFlags::RENDERING)
                .map_err(|err| Error::Allocation(err.to_string()))?,
            false => gbm
                .create_buffer_object_with_modifiers2(
                    width,
                    height,
                    code,
                    named.iter().copied(),
                    BufferObjectFlags::RENDERING,
                )
                .map_err(|err| Error::Allocation(err.to_string()))?,
        };

        if bo.plane_count() != 1 {
            return Err(Error::Allocation(format!(
                "the driver laid the buffer out over {} planes",
                bo.plane_count()
            )));
        }

        let modifier = match named.is_empty() {
            true => MODIFIER_INVALID,
            false => u64::from(bo.modifier()),
        };
        if !named.is_empty() && !modifiers.contains(&modifier) {
            return Err(Error::Allocation(format!(
                "the driver laid the buffer out as {modifier:#x}, which this session does not accept"
            )));
        }
        Ok((bo, modifier))
    }

    pub fn dmabuf_buffer(
        &mut self,
        device: u64,
        format: PixelFormat,
        modifiers: &[u64],
        width: u32,
        height: u32,
    ) -> Result<CaptureBuffer> {
        let dmabuf = self
            .state
            .dmabuf
            .clone()
            .ok_or(Error::Unsupported("zwp_linux_dmabuf_v1"))?;
        let (bo, modifier) = self.allocate(device, format, modifiers, width, height)?;

        let planes: Vec<OwnedPlane> = (0..bo.plane_count() as i32)
            .map(|plane| {
                Ok(OwnedPlane {
                    fd: bo
                        .fd_for_plane(plane)
                        .map_err(|err| Error::Allocation(err.to_string()))?,
                    offset: bo.offset(plane),
                    stride: bo.stride_for_plane(plane),
                })
            })
            .collect::<Result<_>>()?;
        let stride = planes.first().map_or(0, |plane| plane.stride);

        let params = dmabuf.create_params();
        for (index, plane) in planes.iter().enumerate() {
            params.add(
                plane.fd.as_fd(),
                index as u32,
                plane.offset,
                plane.stride,
                (modifier >> 32) as u32,
                (modifier & u64::from(u32::MAX)) as u32,
            );
        }
        let wl = params.create_immed(width as i32, height as i32, format.fourcc(), Flags::empty());
        params.destroy();

        Ok(CaptureBuffer::new(
            wl,
            Memory::Device {
                planes,
                modifier,
                _bo: bo,
            },
            format,
            width,
            height,
            stride,
        ))
    }

    #[must_use]
    pub fn as_fd(&self) -> BorrowedFd<'_> {
        self.event_loop.as_fd()
    }

    pub fn flush(&mut self) -> Result<()> {
        self.event_loop.flush()?;
        Ok(())
    }

    pub(crate) fn source_object(&mut self, source: &Source) -> Result<ExtImageCaptureSourceV1> {
        match source {
            Source::Output(_) | Source::FirstOutput => {
                let manager = self.output_source.as_ref().ok_or(Error::Unsupported(
                    "ext_output_image_capture_source_manager_v1",
                ))?;
                let named = |output: &crate::state::Output| match source {
                    Source::Output(name) => &output.info.name == name,
                    _ => !output.info.name.is_empty(),
                };
                let output = self
                    .state
                    .outputs
                    .iter()
                    .find(|output| named(output))
                    .ok_or_else(|| match source {
                        Source::Output(name) => Error::NoOutput(name.clone()),
                        _ => Error::NoOutput("the first display".into()),
                    })?;
                Ok(manager.create_source(&output.proxy))
            }
            Source::Window(identifier) => {
                let manager = self.window_source.as_ref().ok_or(Error::Unsupported(
                    "ext_foreign_toplevel_image_capture_source_manager_v1",
                ))?;
                let window = self
                    .state
                    .windows
                    .iter()
                    .find(|window| &window.info.identifier == identifier)
                    .ok_or_else(|| Error::NoWindow(identifier.clone()))?;
                Ok(manager.create_source(&window.handle))
            }
        }
    }

    pub fn dispatch(&mut self, deadline: Option<Instant>) -> Result<bool> {
        let turned = self
            .event_loop
            .blocking_dispatch(&mut self.state, |fd| wait_readable(fd, deadline))?;
        Ok(turned)
    }

    pub fn dispatch_with(
        &mut self,
        wait: impl FnOnce(BorrowedFd<'_>) -> std::io::Result<bool>,
    ) -> Result<bool> {
        let turned = self.event_loop.blocking_dispatch(&mut self.state, wait)?;
        Ok(turned)
    }

    fn pointer(&mut self) -> Result<wl_pointer::WlPointer> {
        if let Some(pointer) = &self.pointer {
            return Ok(pointer.clone());
        }
        let seat = self.seat.as_ref().ok_or(Error::Unsupported("wl_seat"))?;
        let pointer = seat.get_pointer();
        Ok(self.pointer.insert(pointer).clone())
    }

    fn device(&mut self, device: u64) -> Result<Arc<gbm::Device<OwnedFd>>> {
        if let Some(open) = self.devices.get(&device) {
            return Ok(Arc::clone(open));
        }
        let node = drm_node(device)?;
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&node)
            .map_err(|err| Error::Allocation(format!("{}: {err}", node.display())))?;
        let gbm = gbm::Device::new(OwnedFd::from(file))
            .map_err(|err| Error::Allocation(format!("{}: {err}", node.display())))?;
        Ok(Arc::clone(
            self.devices.entry(device).or_insert(Arc::new(gbm)),
        ))
    }
}

fn drm_node(device: u64) -> Result<std::path::PathBuf> {
    let nodes = std::fs::read_dir(DRM_NODES)
        .map_err(|err| Error::Allocation(format!("{DRM_NODES}: {err}")))?;
    for node in nodes.flatten() {
        let path = node.path();
        if std::fs::metadata(&path).is_ok_and(|node| node.rdev() == device) {
            return Ok(path);
        }
    }
    Err(Error::Allocation(format!(
        "no device node is {device:#x}, which is where this session allocates a capture"
    )))
}

fn memfd(len: usize) -> Result<OwnedFd> {
    let fd = memfd_create(MEMFD_NAME, MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING)
        .map_err(std::io::Error::from)?;
    ftruncate(&fd, len as u64).map_err(std::io::Error::from)?;
    if let Err(error) = fcntl_add_seals(&fd, SealFlags::SHRINK) {
        tracing::debug!("cannot seal a capture buffer: {error}");
    }
    Ok(fd)
}

impl std::fmt::Debug for Screen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Screen")
            .field("outputs", &self.state.outputs.len())
            .field("windows", &self.state.windows.len())
            .finish_non_exhaustive()
    }
}
