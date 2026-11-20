use std::time::Duration;

use rikka_wayland_client::ext_foreign_toplevel_handle_v1::{self, ExtForeignToplevelHandleV1};
use rikka_wayland_client::ext_foreign_toplevel_list_v1::{self, ExtForeignToplevelListV1};
use rikka_wayland_client::ext_image_copy_capture_cursor_session_v1;
use rikka_wayland_client::ext_image_copy_capture_frame_v1::{self, FailureReason};
use rikka_wayland_client::ext_image_copy_capture_session_v1;
use rikka_wayland_client::zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1;
use rikka_wayland_client::{wl_output, wl_shm};

use crate::format::PixelFormat;
use crate::frames::{CursorSlot, FrameFailure, Slot};
use crate::source::{Region, ScreenOutput, ScreenWindow};

#[derive(Debug)]
pub(crate) struct Output {
    pub(crate) proxy: wl_output::WlOutput,
    pub(crate) info: ScreenOutput,
    transform: wl_output::Transform,
}

impl Output {
    fn resize(&mut self) {
        let turned = matches!(
            self.transform,
            wl_output::Transform::_90
                | wl_output::Transform::_270
                | wl_output::Transform::Flipped90
                | wl_output::Transform::Flipped270
        );
        let (width, height) = match turned {
            true => (self.info.height, self.info.width),
            false => (self.info.width, self.info.height),
        };
        let scale = self.info.scale.max(1) as u32;
        self.info.logical_width = width / scale;
        self.info.logical_height = height / scale;
    }
}

#[derive(Debug)]
pub(crate) struct Window {
    pub(crate) handle: ExtForeignToplevelHandleV1,
    pub(crate) info: ScreenWindow,
}

pub(crate) struct State {
    pub(crate) shm: wl_shm::WlShm,
    pub(crate) dmabuf: Option<ZwpLinuxDmabufV1>,
    pub(crate) outputs: Vec<Output>,
    pub(crate) windows: Vec<Window>,
    pub(crate) changes: u64,
}

impl State {
    pub(crate) fn new(shm: wl_shm::WlShm, dmabuf: Option<ZwpLinuxDmabufV1>) -> Self {
        Self {
            shm,
            dmabuf,
            outputs: Vec::new(),
            windows: Vec::new(),
            changes: 0,
        }
    }

    pub(crate) fn add_output(&mut self, proxy: wl_output::WlOutput) {
        self.outputs.push(Output {
            proxy,
            info: ScreenOutput::new(),
            transform: wl_output::Transform::Normal,
        });
    }

    fn output_mut(&mut self, proxy: &wl_output::WlOutput) -> Option<&mut Output> {
        self.outputs
            .iter_mut()
            .find(|output| &output.proxy == proxy)
    }
}

impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("State")
            .field("outputs", &self.outputs.len())
            .field("windows", &self.windows.len())
            .finish_non_exhaustive()
    }
}

pub(crate) fn on_wl_output(state: &mut State, proxy: wl_output::WlOutput, event: wl_output::Event) {
    state.changes += 1;
    let Some(output) = state.output_mut(&proxy) else {
        return;
    };
    match event {
        wl_output::Event::Name { name } => output.info.name = name,
        wl_output::Event::Description { description } => output.info.description = description,
        wl_output::Event::Geometry {
            x,
            y,
            transform: Ok(transform),
            ..
        } => {
            output.info.x = x;
            output.info.y = y;
            output.transform = transform;
            output.resize();
        }
        wl_output::Event::Mode {
            flags,
            width,
            height,
            refresh,
        } => {
            let current = flags.contains(wl_output::Mode::CURRENT);
            if current {
                output.info.width = width.max(0) as u32;
                output.info.height = height.max(0) as u32;
                output.info.refresh = refresh.max(0) as u32;
                output.resize();
            }
        }
        wl_output::Event::Scale { factor } => {
            output.info.scale = factor.max(1);
            output.resize();
        }
        _ => {}
    }
}

pub(crate) fn on_ext_foreign_toplevel_list_v1(
    state: &mut State,
    _proxy: ExtForeignToplevelListV1,
    event: ext_foreign_toplevel_list_v1::Event,
) {
    state.changes += 1;
    if let ext_foreign_toplevel_list_v1::Event::Toplevel { toplevel } = event {
        state.windows.push(Window {
            handle: toplevel,
            info: ScreenWindow {
                identifier: String::new(),
                title: String::new(),
                app_id: String::new(),
            },
        });
    }
}

pub(crate) fn on_ext_foreign_toplevel_handle_v1(
    state: &mut State,
    proxy: ExtForeignToplevelHandleV1,
    event: ext_foreign_toplevel_handle_v1::Event,
) {
    state.changes += 1;
    if let ext_foreign_toplevel_handle_v1::Event::Closed = event {
        state.windows.retain(|window| window.handle != proxy);
        return;
    }
    let Some(window) = state
        .windows
        .iter_mut()
        .find(|window| window.handle == proxy)
        .map(|window| &mut window.info)
    else {
        return;
    };
    match event {
        ext_foreign_toplevel_handle_v1::Event::Identifier { identifier } => {
            window.identifier = identifier;
        }
        ext_foreign_toplevel_handle_v1::Event::Title { title } => window.title = title,
        ext_foreign_toplevel_handle_v1::Event::AppId { app_id } => window.app_id = app_id,
        _ => {}
    }
}

pub(crate) fn session_event(slot: &Slot, event: ext_image_copy_capture_session_v1::Event) {
    match event {
        ext_image_copy_capture_session_v1::Event::BufferSize { width, height } => {
            slot.size(width, height);
        }
        ext_image_copy_capture_session_v1::Event::ShmFormat { format: Ok(format) } => {
            if let Some(format) = PixelFormat::from_shm(format) {
                slot.shm_format(format);
            }
        }
        ext_image_copy_capture_session_v1::Event::DmabufDevice { device } => {
            if let Some(device) = read_dev(&device) {
                slot.dmabuf_device(device);
            }
        }
        ext_image_copy_capture_session_v1::Event::DmabufFormat { format, modifiers } => {
            if let Some(format) = PixelFormat::from_fourcc(format) {
                slot.dmabuf_format(format, read_modifiers(&modifiers));
            }
        }
        ext_image_copy_capture_session_v1::Event::Done => slot.stated(),
        ext_image_copy_capture_session_v1::Event::Stopped => slot.stopped(),
        _ => {}
    }
}

pub(crate) fn frame_event(slot: &Slot, event: ext_image_copy_capture_frame_v1::Event) {
    match event {
        ext_image_copy_capture_frame_v1::Event::Damage {
            x,
            y,
            width,
            height,
        } => {
            if width > 0 && height > 0 {
                slot.damaged(Region::new(x, y, width as u32, height as u32));
            }
        }
        ext_image_copy_capture_frame_v1::Event::PresentationTime {
            tv_sec_hi,
            tv_sec_lo,
            tv_nsec,
        } => {
            let seconds = (u64::from(tv_sec_hi) << 32) | u64::from(tv_sec_lo);
            slot.presented(Duration::new(seconds, tv_nsec));
        }
        ext_image_copy_capture_frame_v1::Event::Ready => slot.ready(),
        ext_image_copy_capture_frame_v1::Event::Failed { reason } => {
            slot.failed(match reason {
                Ok(FailureReason::BufferConstraints) => FrameFailure::BufferConstraints,
                Ok(FailureReason::Stopped) => FrameFailure::Stopped,
                _ => FrameFailure::Unknown,
            });
        }
        _ => {}
    }
}

pub(crate) fn cursor_event(
    slot: &CursorSlot,
    event: ext_image_copy_capture_cursor_session_v1::Event,
) {
    match event {
        ext_image_copy_capture_cursor_session_v1::Event::Enter => slot.entered(true),
        ext_image_copy_capture_cursor_session_v1::Event::Leave => slot.entered(false),
        ext_image_copy_capture_cursor_session_v1::Event::Position { x, y } => {
            slot.position(x, y);
        }
        ext_image_copy_capture_cursor_session_v1::Event::Hotspot { x, y } => {
            slot.hotspot(x, y);
        }
        _ => {}
    }
}

fn read_dev(bytes: &[u8]) -> Option<u64> {
    bytes
        .get(..size_of::<u64>())
        .and_then(|bytes| bytes.try_into().ok())
        .map(u64::from_ne_bytes)
}

fn read_modifiers(bytes: &[u8]) -> Vec<u64> {
    bytes
        .as_chunks::<{ size_of::<u64>() }>()
        .0
        .iter()
        .map(|modifier| u64::from_ne_bytes(*modifier))
        .collect()
}
