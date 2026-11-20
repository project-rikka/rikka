use std::os::windows::io::AsHandle;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::JoinHandle;

use rikka_worker::{EventFd, EventNotifier, event_fd};
use windows::Win32::Media::Audio::{IAudioClient, IAudioRenderClient, eRender};

use crate::error::{Error, Result, Role};
use crate::mixer::Mixer;
use crate::output::{OutputDevice, OutputPlan};

use super::client::{self, Mode, Stream};
use super::com::{Apartment, Mta};
use super::convert::{Layout, ToDevice};
use super::endpoint::{self, Endpoint, Endpoints};
use super::priority::ProAudio;

const THREAD: &str = "rikka-audio-render";

pub(crate) fn devices() -> Result<Vec<OutputDevice>> {
    let _apartment = Apartment::enter()?;
    let endpoints = Endpoints::new()?;
    let default = endpoints.default_id(eRender)?;
    Ok(endpoints
        .list(eRender)?
        .into_iter()
        .map(|endpoint| OutputDevice {
            is_default: default.as_deref() == Some(endpoint.id.as_str()),
            name: endpoint.name,
        })
        .collect())
}

pub(super) struct Signals {
    pub(super) failed: EventNotifier,
    pub(super) latency: Arc<AtomicU64>,
}

pub(crate) struct Output {
    id: String,
    client: IAudioClient,
    failed: Arc<AtomicBool>,
    stop: EventNotifier,
    stopping: Arc<AtomicBool>,
    thread: Option<JoinHandle<Mixer>>,
}

impl Output {
    pub(super) fn native_rate(endpoint: &Endpoint) -> Result<u32> {
        Ok(endpoint::mix_format(&endpoint.client()?)?.rate)
    }

    pub(super) fn open(
        endpoint: &Endpoint,
        plan: &OutputPlan,
        mixer: Mixer,
        signals: &Signals,
    ) -> Result<Output> {
        let (stream, renderer, event) = Self::prepare(endpoint, plan)?;
        let device = Layout {
            rate: stream.format.rate,
            channels: stream.format.channels,
        };
        let source = Layout {
            rate: plan.rate,
            channels: plan.channels,
        };
        let failed = Arc::new(AtomicBool::new(false));
        let stopping = Arc::new(AtomicBool::new(false));
        let (stop, stopped) = event_fd()?;
        let client = stream.client.clone();
        let rendering = Rendering {
            client: unsafe { Mta::new(stream.client) },
            renderer: unsafe { Mta::new(renderer) },
            event,
            stopped,
            stopping: Arc::clone(&stopping),
            buffer_frames: stream.buffer_frames,
            device_channels: usize::from(device.channels),
            convert: ToDevice::new(source, device, stream.buffer_frames as usize),
            failed: Arc::clone(&failed),
            failure: signals.failed.clone(),
            latency: Arc::clone(&signals.latency),
            device_rate: u64::from(device.rate.max(1)),
            plan_rate: u64::from(plan.rate),
            stream_latency: u64::from(stream.latency_frames),
        };
        let thread = std::thread::Builder::new()
            .name(THREAD.to_owned())
            .spawn(move || rendering.run(mixer))
            .map_err(|source| Error::NoThread {
                name: THREAD,
                source,
            })?;
        let output = Output {
            id: endpoint.id.clone(),
            client,
            failed,
            stop,
            stopping,
            thread: Some(thread),
        };
        output.set_active(true)?;
        Ok(output)
    }

    fn prepare(
        endpoint: &Endpoint,
        plan: &OutputPlan,
    ) -> Result<(Stream, IAudioRenderClient, EventFd)> {
        let format = endpoint::mix_format(&endpoint.client()?)?;
        format.check(Role::Output)?;
        let period = u64::from(plan.quantum) * u64::from(format.rate) / u64::from(plan.rate.max(1));
        let (_, event) = event_fd()?;
        let stream = client::open(
            || endpoint.client(),
            format,
            Mode::Render,
            Some(u32::try_from(period).unwrap_or(u32::MAX)),
            &event,
        )?;
        let renderer = unsafe { stream.client.GetService::<IAudioRenderClient>()? };
        Ok((stream, renderer, event))
    }

    pub(super) fn id(&self) -> &str {
        &self.id
    }

    pub(super) fn has_failed(&self) -> bool {
        self.failed.load(Ordering::Acquire)
    }

    pub(crate) fn set_active(&self, active: bool) -> windows::core::Result<()> {
        match active {
            true => unsafe { self.client.Start() },
            false => unsafe { self.client.Stop() },
        }
    }

    pub(super) fn close(mut self) -> Option<Mixer> {
        self.halt()
    }

    fn halt(&mut self) -> Option<Mixer> {
        let thread = self.thread.take()?;
        self.stopping.store(true, Ordering::Release);
        if let Err(error) = self.stop.notify() {
            tracing::warn!(%error, "failed to stop the audio render thread");
        }
        if let Err(error) = unsafe { self.client.Stop() } {
            tracing::debug!(%error, "failed to stop the audio stream");
        }
        match thread.join() {
            Ok(mixer) => Some(mixer),
            Err(_) => {
                tracing::error!("audio render thread panicked");
                None
            }
        }
    }
}

impl Drop for Output {
    fn drop(&mut self) {
        drop(self.halt());
    }
}

impl std::fmt::Debug for Output {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Output")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

struct Rendering {
    client: Mta<IAudioClient>,
    renderer: Mta<IAudioRenderClient>,
    event: EventFd,
    stopped: EventFd,
    stopping: Arc<AtomicBool>,
    buffer_frames: u32,
    device_channels: usize,
    convert: ToDevice,
    failed: Arc<AtomicBool>,
    failure: EventNotifier,
    latency: Arc<AtomicU64>,
    device_rate: u64,
    plan_rate: u64,
    stream_latency: u64,
}

impl Rendering {
    fn run(mut self, mut mixer: Mixer) -> Mixer {
        let apartment = Apartment::enter();
        if let Err(error) = &apartment {
            tracing::warn!(%error, "audio render thread cannot enter COM");
            self.fail();
            return mixer;
        }
        let _priority = ProAudio::join();
        loop {
            if let Err(error) = self.event.wait_signaled([self.stopped.as_handle()], None) {
                tracing::warn!(%error, "audio render thread cannot wait");
                self.fail();
                break;
            }
            if self.stopping.load(Ordering::Acquire) {
                break;
            }
            self.event.drain();
            if let Err(error) = self.fill(&mut mixer) {
                tracing::warn!(%error, "audio output stream failed");
                self.fail();
                break;
            }
        }
        mixer
    }

    fn fail(&self) {
        self.failed.store(true, Ordering::Release);
        if let Err(error) = self.failure.notify() {
            tracing::warn!(%error, "failed to report a lost audio output");
        }
    }

    fn fill(&mut self, mixer: &mut Mixer) -> windows::core::Result<()> {
        let padding = unsafe { self.client.get().GetCurrentPadding()? };
        let frames = self.buffer_frames.saturating_sub(padding);
        if frames == 0 {
            return Ok(());
        }
        let data = unsafe { self.renderer.get().GetBuffer(frames)? };
        let samples = frames as usize * self.device_channels;
        if data.cast::<f32>().is_aligned() {
            let out = unsafe { std::slice::from_raw_parts_mut(data.cast::<f32>(), samples) };
            self.convert.render(mixer, out);
        } else {
            let out = unsafe { std::slice::from_raw_parts_mut(data, samples * size_of::<f32>()) };
            self.convert.render_unaligned(mixer, out);
        }
        unsafe { self.renderer.get().ReleaseBuffer(frames, 0)? };
        let queued = u64::from(padding + frames) * self.plan_rate / self.device_rate;
        let device = self.stream_latency * self.plan_rate / self.device_rate;
        self.latency.store(queued + device, Ordering::Relaxed);
        Ok(())
    }
}
