use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use pipewire::core::CoreRc;
use pipewire::keys;
use pipewire::properties::properties;
use pipewire::spa::pod::Pod;
use pipewire::spa::utils::Direction;
use pipewire::stream::{Stream, StreamFlags, StreamListener, StreamRc, StreamState};
use rikka_pipewire::{Connection, float_format};

use crate::config::EngineConfig;
use crate::error::{Error, Result};
use crate::lost::DeviceLost;
use crate::mixer::Mixer;
use crate::output::{OutputDevice, OutputPlan};

use super::graph::Graph;

const NAME: &str = "rikka-audio";
const SAMPLE: usize = size_of::<f32>();

pub(crate) fn devices() -> Result<Vec<OutputDevice>> {
    let graph = Graph::read(&Connection::open()?)?;
    Ok(graph
        .outputs()
        .map(|device| OutputDevice {
            name: device.label.clone(),
            is_default: graph.is_default(device),
        })
        .collect())
}

pub(super) fn plan(graph: &Graph, config: &EngineConfig) -> Result<(Option<String>, OutputPlan)> {
    let target = match &config.device_name {
        None => {
            graph.outputs().next().ok_or(Error::NoOutputDevice)?;
            None
        }
        Some(name) => {
            let device = graph
                .find(name)
                .filter(|device| device.plays())
                .ok_or_else(|| Error::DeviceNotFound(name.clone()))?;
            Some(device.node.clone())
        }
    };
    Ok((target, OutputPlan::new(config, graph.clock_rate)?))
}

pub(crate) struct Output {
    stream: StreamRc,
    _listener: StreamListener<()>,
}

impl Output {
    pub(super) fn connect(
        core: &CoreRc,
        target: Option<String>,
        plan: &OutputPlan,
        mixer: Mixer,
        device_lost: Arc<DeviceLost>,
        latency: Arc<AtomicU64>,
    ) -> std::result::Result<Output, pipewire::Error> {
        let mut props = properties! {
            *keys::MEDIA_TYPE => "Audio",
            *keys::MEDIA_CATEGORY => "Playback",
            *keys::NODE_LATENCY => format!("{}/{}", plan.quantum, plan.rate),
        };
        if let Some(target) = target {
            props.insert(*keys::TARGET_OBJECT, target);
        }
        let stream = StreamRc::new(core.clone(), NAME, props)?;

        let mut rendering = Rendering {
            scratch: vec![0.0; mixer.max_block_samples()],
            mixer,
            frame: usize::from(plan.channels) * SAMPLE,
            rate: plan.rate,
            latency,
        };
        let listener = stream
            .add_local_listener_with_user_data(())
            .state_changed(move |_, (), _, state| {
                if let StreamState::Error(reason) = state {
                    tracing::warn!("audio output stream error: {reason}");
                    device_lost.mark();
                }
            })
            .process(move |stream, ()| rendering.fill(stream))
            .register()?;

        let format = float_format(Some(plan.rate), Some(plan.channels));
        let format = Pod::from_bytes(&format).ok_or(pipewire::Error::CreationFailed)?;
        stream.connect(
            Direction::Output,
            None,
            StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS | StreamFlags::RT_PROCESS,
            &mut [format],
        )?;
        Ok(Output {
            stream,
            _listener: listener,
        })
    }

    pub(crate) fn set_active(&self, active: bool) -> std::result::Result<(), pipewire::Error> {
        self.stream.set_active(active)
    }
}

struct Rendering {
    mixer: Mixer,
    scratch: Vec<f32>,
    frame: usize,
    rate: u32,
    latency: Arc<AtomicU64>,
}

impl Rendering {
    fn fill(&mut self, stream: &Stream) {
        let Some(mut buffer) = stream.dequeue_buffer() else {
            return;
        };
        let requested = usize::try_from(buffer.requested()).unwrap_or(usize::MAX);
        let Some(data) = buffer.datas_mut().first_mut() else {
            return;
        };
        let frame = self.frame;
        let filled = match data.data() {
            Some(bytes) => {
                let fitting = bytes.len() / frame;
                let frames = match requested {
                    0 => fitting,
                    requested => requested.min(fitting),
                };
                let area = &mut bytes[..frames * frame];
                self.render(area);
                area.len()
            }
            None => 0,
        };
        let chunk = data.chunk_mut();
        *chunk.offset_mut() = 0;
        *chunk.stride_mut() = i32::try_from(frame).unwrap_or(i32::MAX);
        *chunk.size_mut() = u32::try_from(filled).unwrap_or(u32::MAX);
        drop(buffer);
        if let Ok(time) = stream.time() {
            self.latency.store(self.delay(&time), Ordering::Relaxed);
        }
    }

    fn render(&mut self, area: &mut [u8]) {
        let (unaligned, samples, _) = unsafe { area.align_to_mut::<f32>() };
        if unaligned.is_empty() {
            self.mixer.render(samples);
            return;
        }
        for bytes in area.chunks_mut(self.scratch.len() * SAMPLE) {
            let block = &mut self.scratch[..bytes.len() / SAMPLE];
            self.mixer.render(block);
            for (out, sample) in bytes.as_chunks_mut::<SAMPLE>().0.iter_mut().zip(&*block) {
                *out = sample.to_ne_bytes();
            }
        }
    }

    fn delay(&self, time: &pipewire::stream::Time) -> u64 {
        let clock = time.rate();
        let delay = u64::try_from(time.delay()).unwrap_or(0);
        let device = match clock.denom {
            0 => 0,
            denom => delay * u64::from(clock.num) * u64::from(self.rate) / u64::from(denom),
        };
        device + time.queued() / self.frame as u64 + time.buffered()
    }
}
