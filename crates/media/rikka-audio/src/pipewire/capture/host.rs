use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, mpsc};

use pipewire::keys;
use pipewire::properties::properties;
use pipewire::spa::pod::Pod;
use pipewire::spa::utils::Direction;
use pipewire::stream::{Stream, StreamFlags, StreamRc, StreamState};
use rikka_pipewire::{Connection, LoopWorker, float_format};
use rikka_worker::EventNotifier;

use crate::capture::intake::{Intake, ring};
use crate::capture::{CaptureConfig, Opened};
use crate::error::{Error, Result};
use crate::spawn;

use super::super::graph::Graph;
use super::apps::Linker;
use super::input::{self, InputPlan};

const THREAD: &str = "rikka-audio-capture";
const CONVERTED: usize = 1024;
const SAMPLE: usize = size_of::<f32>();

#[derive(Debug)]
pub(crate) struct CaptureHost {
    _worker: LoopWorker,
    pub(crate) device_lost: Arc<AtomicBool>,
    pub(crate) dropped: Arc<AtomicU64>,
}

impl CaptureHost {
    pub(crate) fn start(
        config: CaptureConfig,
        arrived: EventNotifier,
    ) -> Result<(CaptureHost, Opened)> {
        let device_lost = Arc::new(AtomicBool::new(false));
        let dropped = Arc::new(AtomicU64::new(0));
        let (opened, opening) = mpsc::sync_channel(1);

        let lost = Arc::clone(&device_lost);
        let spilt = Arc::clone(&dropped);
        let worker = rikka_pipewire::spawn(THREAD, move |stopping| {
            let connection = Connection::open()?;
            let plan = input::plan(&Graph::read(&connection)?, &config)?;
            let (producer, consumer) =
                rikka_ring::channel::<f32>(config.capacity(plan.rate, plan.channels));
            let intake = Intake::new(producer, plan.channels, spilt, arrived);
            let stream = connect(&connection, &plan, Taking { intake }, lost)?;
            opened
                .send(Opened {
                    consumer,
                    rate: plan.rate,
                    channels: plan.channels,
                })
                .map_err(|_| Error::ThreadDied { name: THREAD })?;
            Ok(move || {
                connection.run_until_stopped(stopping);
                drop(stream);
            })
        })
        .map_err(spawn::failed(THREAD))?;
        let opened = opening
            .recv()
            .map_err(|_| Error::ThreadDied { name: THREAD })?;

        Ok((
            CaptureHost {
                _worker: worker,
                device_lost,
                dropped,
            },
            opened,
        ))
    }
}

fn connect(
    connection: &Connection,
    plan: &InputPlan,
    mut taking: Taking,
    device_lost: Arc<AtomicBool>,
) -> std::result::Result<impl Sized + use<>, pipewire::Error> {
    let node = plan.node.as_deref().unwrap_or(THREAD);
    let mut props = properties! {
        *keys::MEDIA_TYPE => "Audio",
        *keys::MEDIA_CATEGORY => "Capture",
        *keys::NODE_NAME => node,
    };
    if plan.monitor {
        props.insert(*keys::STREAM_CAPTURE_SINK, "true");
    }
    if plan.passive {
        props.insert(*keys::NODE_PASSIVE, "true");
    }
    if let Some(target) = &plan.target {
        props.insert(*keys::TARGET_OBJECT, target.clone());
    }
    if let Some(period) = plan.period {
        props.insert(*keys::NODE_LATENCY, format!("{period}/{}", plan.rate));
    }
    let stream = StreamRc::new(connection.core().clone(), node, props)?;

    let linker = plan
        .linked
        .clone()
        .map(|linked| Linker::new(connection.core().clone(), linked));
    let watching = linker
        .as_ref()
        .map(|linker| Linker::watch(linker, connection));
    let joined = linker.as_ref().map(Rc::downgrade);
    let failing = Arc::clone(&device_lost);
    let failed = taking.intake.bell();
    let gone = taking.intake.bell();
    let stream_listener = stream
        .add_local_listener_with_user_data(())
        .state_changed(move |stream, (), _, state| match state {
            StreamState::Error(reason) => {
                tracing::warn!("audio input stream error: {reason}");
                failing.store(true, Ordering::Relaxed);
                ring(&failed);
            }
            StreamState::Paused | StreamState::Streaming => {
                if let Some(linker) = &joined {
                    Linker::capture_into(linker, stream.node_id());
                }
            }
            StreamState::Unconnected | StreamState::Connecting => {}
        })
        .process(move |stream, ()| taking.take(stream))
        .register()?;

    let unreachable = Arc::clone(&device_lost);
    let core_listener = connection
        .core()
        .add_listener_local()
        .error(move |id, _, _, message| {
            if id == pipewire::core::PW_ID_CORE {
                tracing::warn!("audio input lost its sound server: {message}");
                unreachable.store(true, Ordering::Relaxed);
                ring(&gone);
            }
        })
        .register();

    let format = float_format(Some(plan.rate), Some(plan.channels));
    let format = Pod::from_bytes(&format).ok_or(pipewire::Error::CreationFailed)?;
    let routing = match linker {
        Some(_) => StreamFlags::empty(),
        None => StreamFlags::AUTOCONNECT,
    };
    stream.connect(
        Direction::Input,
        None,
        routing | StreamFlags::MAP_BUFFERS | StreamFlags::RT_PROCESS,
        &mut [format],
    )?;
    Ok(Connected {
        linker,
        _watching: watching,
        _stream_listener: stream_listener,
        _core_listener: core_listener,
        _stream: stream,
    })
}

struct Connected<W, S, C> {
    linker: Option<Rc<RefCell<Linker>>>,
    _watching: Option<W>,
    _stream_listener: S,
    _core_listener: C,
    _stream: StreamRc,
}

impl<W, S, C> Drop for Connected<W, S, C> {
    fn drop(&mut self) {
        if let Some(linker) = &self.linker {
            linker.borrow_mut().unlink();
        }
    }
}

struct Taking {
    intake: Intake,
}

impl Taking {
    fn take(&mut self, stream: &Stream) {
        let Some(mut buffer) = stream.dequeue_buffer() else {
            return;
        };
        let Some(data) = buffer.datas_mut().first_mut() else {
            return;
        };
        let chunk = data.chunk();
        let (offset, size) = (chunk.offset() as usize, chunk.size() as usize);
        let Some(bytes) = data
            .data()
            .and_then(|mapped| mapped.get(offset..offset.saturating_add(size)))
        else {
            return;
        };
        let channels = self.intake.channels();
        let frame = channels * SAMPLE;
        let block = (CONVERTED - CONVERTED % channels).max(channels);
        let mut converted = [0.0f32; CONVERTED];
        for frames in bytes[..bytes.len() - bytes.len() % frame].chunks(block * SAMPLE) {
            let samples = &mut converted[..(frames.len() / SAMPLE).min(CONVERTED)];
            for (sample, raw) in samples.iter_mut().zip(frames.as_chunks::<SAMPLE>().0) {
                *sample = f32::from_ne_bytes(*raw);
            }
            self.intake.push(samples);
        }
        self.intake.ring();
    }
}
