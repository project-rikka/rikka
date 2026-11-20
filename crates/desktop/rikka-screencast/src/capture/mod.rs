pub(crate) mod format;
pub(crate) mod held;

use std::cell::RefCell;
use std::os::fd::OwnedFd;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use libspa::buffer::meta::{MetaHeader, MetaHeaderFlags, MetaVideoCrop};
use libspa::param::ParamType;
use libspa::pod::Pod;
use libspa::utils::Direction;
use pipewire::channel::Sender;
use pipewire::keys;
use pipewire::properties::properties;
use pipewire::stream::{Stream, StreamFlags, StreamRc, StreamState};
use pipewire::sys as pw_sys;
use rikka_gpu::DrmFormat;
use rikka_pipewire::{Connection, LoopWorker};

use crate::error::{Error, Result};
use crate::frames::{Dmabuf, Frame, Latest, Picture, Pixels};
use format::{NegotiatedFormat, Negotiation};
use held::{Buffers, Command, Held};

const CAPTURE_THREAD: &str = "rikka-screencast";

pub(crate) struct Capture {
    commands: Sender<Command>,
    _worker: LoopWorker,
}

pub(crate) struct StreamRequest {
    pub(crate) node: u32,
    pub(crate) frame_rate: u32,
    pub(crate) size: Option<(u32, u32)>,
}

impl Capture {
    pub(crate) fn start(
        remote: OwnedFd,
        request: StreamRequest,
        latest: Arc<Latest>,
    ) -> Result<Capture> {
        let (commands, received) = pipewire::channel::channel();
        let returns = commands.clone();
        let worker = rikka_pipewire::spawn(CAPTURE_THREAD, move |stopping| {
            connect_stream(remote, request, latest, returns, received, stopping)
        })?;
        Ok(Capture {
            commands,
            _worker: worker,
        })
    }

    pub(crate) fn offer_dmabufs(&self, formats: Vec<DrmFormat>) {
        if self.commands.send(Command::Offer(formats)).is_err() {
            tracing::debug!("the screen capture ended before it could renegotiate");
        }
    }
}

struct ReceiverState {
    latest: Arc<Latest>,
    format: Option<NegotiatedFormat>,
    buffers: Rc<RefCell<Buffers>>,
    returns: Sender<Command>,
}

fn connect_stream(
    remote: OwnedFd,
    request: StreamRequest,
    latest: Arc<Latest>,
    returns: Sender<Command>,
    received: pipewire::channel::Receiver<Command>,
    stopping: pipewire::channel::Receiver<()>,
) -> Result<impl FnOnce()> {
    let connection = Connection::open_fd(remote).map_err(Error::pipewire)?;
    let main_loop = connection.main_loop();
    let core = connection.core();

    let latest_on_error = Arc::clone(&latest);
    let weak_loop = main_loop.downgrade();
    let core_listener = core
        .add_listener_local()
        .error(move |id, _, _, message| {
            if id == 0 {
                latest_on_error.end(Some(message.to_owned()));
                if let Some(main_loop) = weak_loop.upgrade() {
                    main_loop.quit();
                }
            }
        })
        .register();

    let stream = StreamRc::new(
        core.clone(),
        CAPTURE_THREAD,
        properties! {
            *keys::MEDIA_TYPE => "Video",
            *keys::MEDIA_CATEGORY => "Capture",
            *keys::MEDIA_ROLE => "Screen",
        },
    )
    .map_err(Error::pipewire)?;

    let buffers = Rc::new(RefCell::new(Buffers::default()));
    let (added, removed) = (Rc::clone(&buffers), Rc::clone(&buffers));
    let ending = main_loop.downgrade();
    let stream_listener = stream
        .add_local_listener_with_user_data(ReceiverState {
            latest: Arc::clone(&latest),
            format: None,
            buffers: Rc::clone(&buffers),
            returns,
        })
        .state_changed(move |_, receiver, _, state| match state {
            StreamState::Error(reason) => {
                receiver.latest.end(Some(reason));
                if let Some(main_loop) = ending.upgrade() {
                    main_loop.quit();
                }
            }
            StreamState::Unconnected => {
                receiver.latest.end(None);
                if let Some(main_loop) = ending.upgrade() {
                    main_loop.quit();
                }
            }
            StreamState::Connecting | StreamState::Paused | StreamState::Streaming => {}
        })
        .param_changed(on_param_changed)
        .add_buffer(move |_, _, buffer| added.borrow_mut().add(buffer))
        .remove_buffer(move |_, _, buffer| removed.borrow_mut().remove(buffer))
        .process(process_buffer)
        .register()
        .map_err(Error::pipewire)?;

    let offered = format::offers(request.frame_rate, request.size, &[]);
    let mut params: Vec<&Pod> = offered
        .iter()
        .map(|bytes| {
            Pod::from_bytes(bytes).ok_or_else(|| Error::pipewire("invalid video format pod"))
        })
        .collect::<Result<_>>()?;
    stream
        .connect(
            Direction::Input,
            Some(request.node),
            StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS,
            &mut params,
        )
        .map_err(Error::pipewire)?;

    let commanded = stream.clone();
    let (frame_rate, size) = (request.frame_rate, request.size);
    let act = move |command| match command {
        Command::Return(key) => {
            if let Some(buffer) = buffers.borrow().returned(key) {
                unsafe { commanded.queue_raw_buffer(buffer) };
            }
        }
        Command::Offer(formats) => {
            let offered = format::offers(frame_rate, size, &formats);
            let mut params: Vec<&Pod> = offered
                .iter()
                .filter_map(|bytes| Pod::from_bytes(bytes))
                .collect();
            if let Err(error) = commanded.update_params(&mut params) {
                tracing::warn!(%error, "failed to offer shared screen buffers");
            }
        }
    };

    Ok(move || {
        let commands = received.attach(connection.main_loop().loop_(), act);
        connection.run_until_stopped(stopping);
        if let Err(error) = stream.disconnect() {
            tracing::debug!(%error, "failed to disconnect screen capture stream");
        }
        drop(commands);
        drop((stream_listener, stream, core_listener, connection));
        latest.end(None);
    })
}

fn on_param_changed(stream: &Stream, receiver: &mut ReceiverState, id: u32, param: Option<&Pod>) {
    if id != ParamType::Format.as_raw() {
        return;
    }
    let Some(param) = param else {
        receiver.format = None;
        return;
    };
    receiver.format = match format::read(param) {
        Some(Negotiation::Settled(negotiated)) => Some(negotiated),
        Some(Negotiation::Unsettled) | None => None,
    };
    let Some(negotiated) = receiver.format else {
        return;
    };
    let buffers = format::buffers(negotiated.modifier.is_some());
    let metas = format::metas();
    let mut params: Vec<&Pod> = std::iter::once(&buffers)
        .chain(metas.iter())
        .filter_map(|bytes| Pod::from_bytes(bytes))
        .collect();
    if let Err(error) = stream.update_params(&mut params) {
        tracing::warn!(%error, "failed to update stream buffer parameters");
    }
}

fn process_buffer(stream: &Stream, receiver: &mut ReceiverState) {
    match receiver.format {
        Some(format) if format.modifier.is_some() => shared_buffer(stream, receiver, format),
        Some(format) => copied_buffer(stream, receiver, format),
        None => while stream.dequeue_buffer().is_some() {},
    }
}

fn shared_buffer(stream: &Stream, receiver: &mut ReceiverState, format: NegotiatedFormat) {
    let mut newest: Option<*mut pw_sys::pw_buffer> = None;
    loop {
        let buffer = unsafe { stream.dequeue_raw_buffer() };
        if buffer.is_null() {
            break;
        }
        if let Some(older) = newest.replace(buffer) {
            unsafe { stream.queue_raw_buffer(older) };
        }
    }
    let Some(buffer) = newest else {
        return;
    };
    let shared = match unsafe { held::corrupted(buffer) } {
        true => None,
        false => receiver
            .buffers
            .borrow_mut()
            .shared(buffer)
            .zip(unsafe { held::planes(buffer) }),
    };
    let Some(((key, fds), planes)) = shared else {
        unsafe { stream.queue_raw_buffer(buffer) };
        return;
    };
    receiver.latest.publish(Frame {
        width: format.width,
        height: format.height,
        captured: Instant::now(),
        picture: Picture::Dmabuf(Dmabuf {
            key,
            fourcc: format.fourcc(),
            modifier: format.modifier.unwrap_or_default(),
            fds,
            planes,
            _held: Held::new(key, receiver.returns.clone()),
        }),
    });
}

fn copied_buffer(stream: &Stream, receiver: &mut ReceiverState, format: NegotiatedFormat) {
    let mut newest = None;
    while let Some(buffer) = stream.dequeue_buffer() {
        newest = Some(buffer);
    }
    let Some(mut buffer) = newest else {
        return;
    };
    let corrupted = buffer
        .find_meta::<MetaHeader>()
        .is_some_and(|header| header.flags().contains(MetaHeaderFlags::CORRUPTED));
    if corrupted {
        return;
    }
    let crop = buffer
        .find_meta::<MetaVideoCrop>()
        .map(|crop| crop.meta_region().clone())
        .filter(|region| region.is_valid())
        .map(|region| {
            let (position, size) = (region.position(), region.size());
            (
                position.x.max(0) as u32,
                position.y.max(0) as u32,
                size.width,
                size.height,
            )
        });
    let Some(data) = buffer.datas_mut().first_mut() else {
        return;
    };
    let chunk = data.chunk();
    let (offset, size, stride) = (
        chunk.offset() as usize,
        chunk.size() as usize,
        chunk.stride(),
    );
    if size == 0 {
        return;
    }
    let stride = match usize::try_from(stride) {
        Ok(stride) if stride > 0 => stride,
        _ => format.width as usize * 4,
    };
    let Some(mapped) = data.data() else {
        return;
    };
    let Some(bytes) = mapped.get(offset..offset.saturating_add(size)) else {
        return;
    };
    let (x, y, width, height) = crop_rect(format, crop);
    if width == 0 || height == 0 {
        return;
    }
    let row = width as usize * 4;
    let mut pixels = receiver.latest.take_buffer(row * height as usize);
    for line in 0..height as usize {
        let start = (y as usize + line) * stride + x as usize * 4;
        let Some(source) = bytes.get(start..start + row) else {
            receiver.latest.recycle(pixels);
            return;
        };
        pixels.extend_from_slice(source);
    }
    receiver.latest.publish(Frame {
        width,
        height,
        captured: Instant::now(),
        picture: Picture::Pixels(Pixels {
            order: format.order,
            stride: row,
            bytes: pixels,
        }),
    });
}

fn crop_rect(format: NegotiatedFormat, crop: Option<(u32, u32, u32, u32)>) -> (u32, u32, u32, u32) {
    let whole = (0, 0, format.width, format.height);
    let Some((x, y, width, height)) = crop else {
        return whole;
    };
    if x >= format.width || y >= format.height || width == 0 || height == 0 {
        return whole;
    }
    let width = width.min(format.width - x) & !1;
    let height = height.min(format.height - y) & !1;
    (x, y, width, height)
}
