use std::os::windows::io::AsHandle;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use rikka_worker::{EventFd, EventNotifier, Worker, event_fd};
use windows::Win32::Media::Audio::{
    AUDCLNT_BUFFERFLAGS_SILENT, EDataFlow, IAudioCaptureClient, IAudioClient, IAudioRenderClient,
    PROCESS_LOOPBACK_MODE, PROCESS_LOOPBACK_MODE_EXCLUDE_TARGET_PROCESS_TREE,
    PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE, eCapture, eRender,
};

use crate::capture::intake::Intake;
use crate::capture::{AudioApp, CaptureConfig, CaptureDevice, CaptureKind, CaptureSource, Opened};
use crate::error::{Error, Result, Role};
use crate::frame::check_channels;
use crate::output::{DEFAULT_CHANNELS, DEFAULT_RATE};
use crate::spawn;

use super::client::{self, Mode};
use super::com::Apartment;
use super::convert::{FromDevice, Layout};
use super::endpoint::{self, Endpoint, Endpoints};
use super::format::DeviceFormat;
use super::host::Stopper;
use super::sessions::{self, SessionWatch};
use super::{loopback, notify};

const THREAD: &str = "rikka-audio-capture";
const TAP_IDLE: Duration = Duration::from_millis(50);

pub(crate) fn devices() -> Result<Vec<CaptureDevice>> {
    let _apartment = Apartment::enter()?;
    let endpoints = Endpoints::new()?;
    let mut devices = Vec::new();
    for (flow, kind) in [
        (eCapture, CaptureKind::Microphone),
        (eRender, CaptureKind::SystemAudio),
    ] {
        let default = endpoints.default_id(flow)?;
        devices.extend(
            endpoints
                .list(flow)?
                .into_iter()
                .map(|endpoint| CaptureDevice {
                    is_default: default.as_deref() == Some(endpoint.id.as_str()),
                    name: endpoint.name,
                    kind,
                }),
        );
    }
    Ok(devices)
}

pub(crate) fn apps() -> Result<Vec<AudioApp>> {
    let _apartment = Apartment::enter()?;
    Ok(sessions::apps(&Endpoints::new()?)?)
}

#[derive(Debug)]
pub(crate) struct CaptureHost {
    _worker: Worker<Stopper>,
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
        let (stopper, stopping, stopped) = Stopper::new()?;
        let (opened, opening) = mpsc::sync_channel(1);
        let lost = Arc::clone(&device_lost);
        let spilt = Arc::clone(&dropped);
        let worker = Worker::spawn(THREAD, stopper, move || {
            let apartment = Apartment::enter()?;
            let endpoints = Endpoints::new()?;
            let (changed_bell, changed) = event_fd()?;
            let route = CaptureRoute::new(&config.source);
            let native = route.native_rate(&endpoints)?;
            let target = Layout {
                rate: config
                    .sample_rate
                    .or(native)
                    .filter(|rate| *rate > 0)
                    .unwrap_or(DEFAULT_RATE),
                channels: config.channels.unwrap_or(DEFAULT_CHANNELS),
            };
            check_channels(target.channels, Role::Input)?;
            let tuning = Tuning {
                target,
                period: config.period,
                keep_alive: !config.passive,
            };
            let source = route.open(&endpoints, &tuning, &changed_bell)?;
            let (producer, consumer) =
                rikka_ring::channel::<f32>(config.capacity(target.rate, target.channels));
            let intake = Intake::new(producer, target.channels, spilt, arrived);
            opened
                .send(Opened {
                    consumer,
                    rate: target.rate,
                    channels: target.channels,
                })
                .map_err(|_| Error::ThreadDied { name: THREAD })?;
            Ok(move || {
                let mut capturing = Capturing {
                    endpoints,
                    route,
                    tuning,
                    intake,
                    lost,
                    changed_bell,
                };
                capturing.run(source, &stopping, &stopped, &changed);
                drop(capturing);
                drop(apartment);
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

struct Tuning {
    target: Layout,
    period: Option<u32>,
    keep_alive: bool,
}

enum CaptureRoute {
    Default { flow: EDataFlow },
    Named(String),
    OtherApps,
    Apps(Vec<String>),
}

impl CaptureRoute {
    fn new(source: &CaptureSource) -> CaptureRoute {
        match source {
            CaptureSource::Microphone => CaptureRoute::Default { flow: eCapture },
            CaptureSource::SystemAudio => CaptureRoute::Default { flow: eRender },
            CaptureSource::Named(name) => CaptureRoute::Named(name.clone()),
            CaptureSource::OtherApps => CaptureRoute::OtherApps,
            CaptureSource::Apps(keys) => CaptureRoute::Apps(keys.clone()),
        }
    }

    fn endpoint(&self, endpoints: &Endpoints) -> Result<Option<(Endpoint, Mode)>> {
        match self {
            CaptureRoute::Default { flow } => {
                let (missing, mode) = match *flow {
                    flow if flow == eCapture => (Error::NoInputDevice, Mode::Capture),
                    _ => (Error::NoSystemAudio, Mode::Loopback),
                };
                let endpoint = endpoints.default(*flow)?.ok_or(missing)?;
                Ok(Some((endpoint, mode)))
            }
            CaptureRoute::Named(name) => {
                if let Some(endpoint) = endpoints.find(eCapture, name)? {
                    return Ok(Some((endpoint, Mode::Capture)));
                }
                let endpoint = endpoints
                    .find(eRender, name)?
                    .ok_or_else(|| Error::DeviceNotFound(name.clone()))?;
                Ok(Some((endpoint, Mode::Loopback)))
            }
            CaptureRoute::OtherApps | CaptureRoute::Apps(_) => Ok(None),
        }
    }

    fn native_rate(&self, endpoints: &Endpoints) -> Result<Option<u32>> {
        let endpoint = match self.endpoint(endpoints)? {
            Some((endpoint, _)) => Some(endpoint),
            None => endpoints.default(eRender)?,
        };
        match endpoint {
            Some(endpoint) => Ok(Some(endpoint::mix_format(&endpoint.client()?)?.rate)),
            None => Ok(None),
        }
    }

    fn open(
        &self,
        endpoints: &Endpoints,
        tuning: &Tuning,
        changed: &EventNotifier,
    ) -> Result<Source> {
        if let Some((endpoint, mode)) = self.endpoint(endpoints)? {
            return Ok(Source::Device(Box::new(DeviceTap::open(
                &endpoint, mode, tuning, endpoints, changed,
            )?)));
        }
        let keys = match self {
            CaptureRoute::Apps(keys) => Some(keys.clone()),
            _ => None,
        };
        Ok(Source::Processes(ProcessTaps::open(
            keys,
            tuning.target,
            endpoints,
            changed,
        )?))
    }
}

enum Source {
    Device(Box<DeviceTap>),
    Processes(ProcessTaps),
}

impl Source {
    fn event(&self) -> &EventFd {
        match self {
            Source::Device(tap) => &tap.event,
            Source::Processes(taps) => &taps.event,
        }
    }

    fn pump(&mut self, intake: &mut Intake) -> windows::core::Result<()> {
        match self {
            Source::Device(tap) => tap.pump(intake),
            Source::Processes(taps) => taps.pump(intake),
        }
    }
}

struct Capturing {
    endpoints: Endpoints,
    route: CaptureRoute,
    tuning: Tuning,
    intake: Intake,
    lost: Arc<AtomicBool>,
    changed_bell: EventNotifier,
}

impl Capturing {
    fn run(
        &mut self,
        mut source: Source,
        stopping: &AtomicBool,
        stopped: &EventFd,
        changed: &EventFd,
    ) {
        loop {
            let waited = source
                .event()
                .wait_signaled([stopped.as_handle(), changed.as_handle()], None);
            if stopping.load(Ordering::Acquire) {
                return;
            }
            let [_, rerouted] = match waited {
                Ok(signaled) => signaled,
                Err(error) => {
                    tracing::warn!(%error, "audio capture cannot wait");
                    return self.lose();
                }
            };
            source.event().drain();
            let failed = match source.pump(&mut self.intake) {
                Ok(()) => false,
                Err(error) => {
                    tracing::warn!(%error, "audio input stream failed");
                    true
                }
            };
            if rerouted || failed {
                changed.drain();
                source = match self.reroute(source, failed) {
                    Some(source) => source,
                    None => {
                        self.lose();
                        return wait_for_stop(stopping, stopped);
                    }
                };
            }
        }
    }

    fn reroute(&mut self, source: Source, failed: bool) -> Option<Source> {
        match (source, &self.route) {
            (Source::Processes(mut taps), _) if !failed => {
                taps.refresh(&self.endpoints);
                Some(Source::Processes(taps))
            }
            (Source::Device(tap), CaptureRoute::Default { flow }) if !failed => {
                match self.endpoints.default_id(*flow) {
                    Ok(Some(id)) if id == tap.id => Some(Source::Device(tap)),
                    Ok(_) => {
                        drop(tap);
                        self.reopen()
                    }
                    Err(error) => {
                        tracing::warn!(%error, "failed to find the default audio input");
                        None
                    }
                }
            }
            (Source::Device(tap), CaptureRoute::Named(_)) if !failed => {
                match self.endpoints.is_active(eCapture, &tap.id) {
                    Ok(true) => Some(Source::Device(tap)),
                    Ok(false) => match self.endpoints.is_active(eRender, &tap.id) {
                        Ok(true) => Some(Source::Device(tap)),
                        Ok(false) => None,
                        Err(error) => {
                            tracing::warn!(%error, "failed to check the audio input device");
                            None
                        }
                    },
                    Err(error) => {
                        tracing::warn!(%error, "failed to check the audio input device");
                        None
                    }
                }
            }
            (source, CaptureRoute::Default { .. }) => {
                drop(source);
                self.reopen()
            }
            (source, CaptureRoute::Named(_) | CaptureRoute::OtherApps | CaptureRoute::Apps(_)) => {
                drop(source);
                None
            }
        }
    }

    fn reopen(&self) -> Option<Source> {
        match self
            .route
            .open(&self.endpoints, &self.tuning, &self.changed_bell)
        {
            Ok(source) => {
                tracing::info!("audio input moved");
                Some(source)
            }
            Err(error) => {
                tracing::warn!(%error, "failed to reopen the audio input");
                None
            }
        }
    }

    fn lose(&self) {
        self.lost.store(true, Ordering::Relaxed);
        self.intake.ring();
    }
}

fn wait_for_stop(stopping: &AtomicBool, stopped: &EventFd) {
    while !stopping.load(Ordering::Acquire) {
        if let Err(error) = stopped.wait() {
            tracing::warn!(%error, "audio capture cannot wait");
            return;
        }
        stopped.drain();
    }
}

struct DeviceTap {
    id: String,
    client: IAudioClient,
    capture: IAudioCaptureClient,
    event: EventFd,
    channels: usize,
    convert: FromDevice,
    silence: Vec<f32>,
    scratch: Vec<f32>,
    keep_alive: Option<KeepAlive>,
    _registration: endpoint::Registration,
}

impl DeviceTap {
    fn open(
        endpoint: &Endpoint,
        mode: Mode,
        tuning: &Tuning,
        endpoints: &Endpoints,
        changed: &EventNotifier,
    ) -> Result<DeviceTap> {
        let registration = endpoints.register(notify::listener(changed.clone()))?;
        let format = endpoint::mix_format(&endpoint.client()?)?;
        format.check(Role::Input)?;
        let device = format;
        let period = tuning.period.map(|frames| {
            let scaled =
                u64::from(frames) * u64::from(device.rate) / u64::from(tuning.target.rate.max(1));
            u32::try_from(scaled).unwrap_or(u32::MAX)
        });
        let keep_alive = match (mode, tuning.keep_alive) {
            (Mode::Loopback, true) => Some(KeepAlive::start(endpoint, device)?),
            _ => None,
        };
        let (_, event) = event_fd()?;
        let stream = client::open(|| endpoint.client(), device, mode, period, &event)?;
        let capture = unsafe { stream.client.GetService::<IAudioCaptureClient>()? };
        unsafe { stream.client.Start()? };
        Ok(DeviceTap {
            id: endpoint.id.clone(),
            client: stream.client,
            capture,
            event,
            channels: usize::from(device.channels),
            convert: FromDevice::new(
                Layout {
                    rate: device.rate,
                    channels: device.channels,
                },
                tuning.target,
            ),
            silence: Vec::new(),
            scratch: Vec::new(),
            keep_alive,
            _registration: registration,
        })
    }

    fn pump(&mut self, intake: &mut Intake) -> windows::core::Result<()> {
        while unsafe { self.capture.GetNextPacketSize()? } > 0 {
            let mut data = std::ptr::null_mut();
            let mut frames = 0;
            let mut flags = 0;
            unsafe {
                self.capture
                    .GetBuffer(&mut data, &mut frames, &mut flags, None, None)?;
            }
            let samples = frames as usize * self.channels;
            let silent = flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 || data.is_null();
            let input: &[f32] = if silent {
                self.silence.resize(samples, 0.0);
                &self.silence[..samples]
            } else if data.cast::<f32>().is_aligned() {
                unsafe { std::slice::from_raw_parts(data.cast::<f32>(), samples) }
            } else {
                let bytes = unsafe { std::slice::from_raw_parts(data, samples * size_of::<f32>()) };
                self.scratch.clear();
                self.scratch.extend(
                    bytes
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .map(|raw| f32::from_ne_bytes(*raw)),
                );
                &self.scratch
            };
            intake.push(self.convert.convert(input, frames as usize));
            unsafe { self.capture.ReleaseBuffer(frames)? };
        }
        if let Some(keep_alive) = &self.keep_alive {
            keep_alive.top_up()?;
        }
        intake.ring();
        Ok(())
    }
}

impl Drop for DeviceTap {
    fn drop(&mut self) {
        if let Err(error) = unsafe { self.client.Stop() } {
            tracing::debug!(%error, "failed to stop the audio input stream");
        }
    }
}

struct KeepAlive {
    client: IAudioClient,
    renderer: IAudioRenderClient,
    buffer_frames: u32,
    _event: EventFd,
}

impl KeepAlive {
    fn start(endpoint: &Endpoint, format: DeviceFormat) -> Result<KeepAlive> {
        let (_, event) = event_fd()?;
        let stream = client::open(|| endpoint.client(), format, Mode::Render, None, &event)?;
        let renderer = unsafe { stream.client.GetService::<IAudioRenderClient>()? };
        let keep_alive = KeepAlive {
            client: stream.client,
            renderer,
            buffer_frames: stream.buffer_frames,
            _event: event,
        };
        keep_alive.top_up()?;
        unsafe { keep_alive.client.Start()? };
        Ok(keep_alive)
    }

    fn top_up(&self) -> windows::core::Result<()> {
        let padding = unsafe { self.client.GetCurrentPadding()? };
        let free = self.buffer_frames.saturating_sub(padding);
        if free > 0 {
            unsafe {
                self.renderer.GetBuffer(free)?;
                self.renderer
                    .ReleaseBuffer(free, AUDCLNT_BUFFERFLAGS_SILENT.0 as u32)?;
            }
        }
        Ok(())
    }
}

impl Drop for KeepAlive {
    fn drop(&mut self) {
        if let Err(error) = unsafe { self.client.Stop() } {
            tracing::debug!(%error, "failed to stop the system audio keep-alive stream");
        }
    }
}

struct ProcessTap {
    process: u32,
    client: IAudioClient,
    capture: IAudioCaptureClient,
    queued: Vec<f32>,
    heard: Instant,
}

impl ProcessTap {
    fn open(
        process: u32,
        mode: PROCESS_LOOPBACK_MODE,
        target: Layout,
        event: &EventFd,
    ) -> Result<ProcessTap> {
        let activated = loopback::activate(process, mode)?;
        let mut client = Some(activated);
        let stream = client::open(
            || {
                client.take().ok_or_else(|| {
                    windows::core::Error::from(windows::Win32::Foundation::E_UNEXPECTED)
                })
            },
            DeviceFormat::float32(target.rate, target.channels),
            Mode::ProcessLoopback,
            None,
            event,
        )?;
        let capture = unsafe { stream.client.GetService::<IAudioCaptureClient>()? };
        unsafe { stream.client.Start()? };
        Ok(ProcessTap {
            process,
            client: stream.client,
            capture,
            queued: Vec::new(),
            heard: Instant::now(),
        })
    }

    fn take(&mut self, channels: usize, now: Instant) -> windows::core::Result<()> {
        while unsafe { self.capture.GetNextPacketSize()? } > 0 {
            let mut data = std::ptr::null_mut();
            let mut frames = 0;
            let mut flags = 0;
            unsafe {
                self.capture
                    .GetBuffer(&mut data, &mut frames, &mut flags, None, None)?;
            }
            let samples = frames as usize * channels;
            let silent = flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 || data.is_null();
            if silent {
                self.queued.resize(self.queued.len() + samples, 0.0);
            } else {
                let bytes = unsafe { std::slice::from_raw_parts(data, samples * size_of::<f32>()) };
                self.queued.extend(
                    bytes
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .map(|raw| f32::from_ne_bytes(*raw)),
                );
            }
            unsafe { self.capture.ReleaseBuffer(frames)? };
            if frames > 0 {
                self.heard = now;
            }
        }
        Ok(())
    }
}

impl Drop for ProcessTap {
    fn drop(&mut self) {
        if let Err(error) = unsafe { self.client.Stop() } {
            tracing::debug!(%error, process = self.process, "failed to stop an app capture stream");
        }
    }
}

struct ProcessTaps {
    keys: Option<Vec<String>>,
    target: Layout,
    event: EventFd,
    taps: Vec<ProcessTap>,
    mixed: Vec<f32>,
    _sessions: Option<SessionWatch>,
}

impl ProcessTaps {
    fn open(
        keys: Option<Vec<String>>,
        target: Layout,
        endpoints: &Endpoints,
        changed: &EventNotifier,
    ) -> Result<ProcessTaps> {
        let (_, event) = event_fd()?;
        let sessions = match keys {
            Some(_) => Some(SessionWatch::start(endpoints, changed.clone())?),
            None => None,
        };
        let mut taps = ProcessTaps {
            keys,
            target,
            event,
            taps: Vec::new(),
            mixed: Vec::new(),
            _sessions: sessions,
        };
        match taps.keys {
            None => taps.taps.push(ProcessTap::open(
                std::process::id(),
                PROCESS_LOOPBACK_MODE_EXCLUDE_TARGET_PROCESS_TREE,
                target,
                &taps.event,
            )?),
            Some(_) => taps.refresh(endpoints),
        }
        Ok(taps)
    }

    fn refresh(&mut self, endpoints: &Endpoints) {
        let Some(keys) = &self.keys else {
            return;
        };
        let wanted: Vec<u32> = match sessions::list(endpoints) {
            Ok(found) => found
                .into_iter()
                .filter(|session| keys.contains(&session.key))
                .map(|session| session.process)
                .collect(),
            Err(error) => {
                tracing::warn!(%error, "failed to list audio sessions");
                return;
            }
        };
        self.taps.retain(|tap| wanted.contains(&tap.process));
        for process in wanted {
            if self.taps.iter().any(|tap| tap.process == process) {
                continue;
            }
            match ProcessTap::open(
                process,
                PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
                self.target,
                &self.event,
            ) {
                Ok(tap) => self.taps.push(tap),
                Err(error) => tracing::warn!(%error, process, "failed to capture an app"),
            }
        }
    }

    fn pump(&mut self, intake: &mut Intake) -> windows::core::Result<()> {
        let channels = usize::from(self.target.channels);
        let now = Instant::now();
        for tap in &mut self.taps {
            tap.take(channels, now)?;
        }
        let sounding = |tap: &&ProcessTap| now.saturating_duration_since(tap.heard) < TAP_IDLE;
        let ready = match self
            .taps
            .iter()
            .filter(sounding)
            .map(|tap| tap.queued.len())
            .min()
        {
            Some(ready) => ready,
            None => self
                .taps
                .iter()
                .map(|tap| tap.queued.len())
                .max()
                .unwrap_or(0),
        };
        let ready = ready - ready % channels;
        if ready == 0 {
            return Ok(());
        }
        self.mixed.clear();
        self.mixed.resize(ready, 0.0);
        for tap in &mut self.taps {
            let taken = ready.min(tap.queued.len());
            for (mixed, sample) in self.mixed.iter_mut().zip(tap.queued.drain(..taken)) {
                *mixed += sample;
            }
        }
        intake.push(&self.mixed);
        intake.ring();
        Ok(())
    }
}
