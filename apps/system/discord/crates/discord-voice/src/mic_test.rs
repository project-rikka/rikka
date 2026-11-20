use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use futures_channel::mpsc;
use rikka_worker::{EventFd, EventNotifier, Stop, Worker, event_fd};

use crate::error::{Error, Result};
use crate::input::{Chain, STEP, STEP_SAMPLES};
use crate::io::{Capture, Devices, Playback, System};
use crate::krisp::Krisp;
use crate::meter::Meter;
use crate::output::OUTPUT_CHANNELS;
use crate::settings::VoiceSettings;
use crate::worker::spawn;

const LOOPBACK_QUEUE: Duration = Duration::from_millis(120);
const TEST_THREAD: &str = "rikka-discord-voice-test";

pub type Levels = mpsc::UnboundedReceiver<f32>;

pub struct MicTest {
    levels: Option<Levels>,
    worker: Worker<Arc<Control>>,
}

struct Control {
    loopback: AtomicBool,
    stop: AtomicBool,
    bell: EventNotifier,
}

impl Control {
    fn ring(&self) {
        if let Err(error) = self.bell.notify() {
            tracing::warn!(%error, "failed to wake the microphone test");
        }
    }
}

impl Stop for Control {
    fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
        self.ring();
    }
}

impl MicTest {
    pub fn start(settings: &VoiceSettings, krisp: &Krisp) -> Result<MicTest> {
        let devices: Arc<dyn Devices> = Arc::new(System::start()?);
        let mut capture = Capture::new(Arc::clone(&devices), settings.input.clone(), STEP_SAMPLES);
        capture.open()?;
        let (bell, rung) = event_fd().map_err(|source| Error::Thread {
            name: TEST_THREAD,
            source,
        })?;
        devices.hotplug().watch(bell.clone());
        let (sender, levels) = mpsc::unbounded();
        let (chain, _) = Chain::new(settings, krisp.clone());
        let testing = Testing {
            capture,
            chain,
            meter: Meter::new(STEP),
            playback: Playback::new(devices, settings.output.clone()),
            volume: settings.output_gain(),
            stereo: Vec::with_capacity(STEP_SAMPLES * usize::from(OUTPUT_CHANNELS)),
            levels: sender,
        };
        let control = Arc::new(Control {
            loopback: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            bell,
        });
        let controlled = Arc::clone(&control);
        let worker = spawn(TEST_THREAD, control, move || {
            testing.run(&controlled, &rung);
        })?;
        Ok(MicTest {
            levels: Some(levels),
            worker,
        })
    }

    pub fn levels(&mut self) -> Levels {
        self.levels.take().unwrap_or_else(|| mpsc::unbounded().1)
    }

    pub fn set_loopback(&self, on: bool) {
        let control = self.worker.handle();
        control.loopback.store(on, Ordering::Relaxed);
        control.ring();
    }
}

impl std::fmt::Debug for MicTest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MicTest")
            .field(
                "loopback",
                &self.worker.handle().loopback.load(Ordering::Relaxed),
            )
            .finish_non_exhaustive()
    }
}

struct Testing {
    capture: Capture,
    chain: Chain,
    meter: Meter,
    playback: Playback,
    volume: f32,
    stereo: Vec<f32>,
    levels: mpsc::UnboundedSender<f32>,
}

impl Testing {
    fn run(mut self, control: &Control, bell: &EventFd) {
        let mut step = [0.0; STEP_SAMPLES];
        loop {
            bell.drain();
            if control.stop.load(Ordering::Relaxed) {
                return;
            }
            let looping = control.loopback.load(Ordering::Relaxed);
            if looping {
                self.playback.poll();
            } else {
                self.playback.release();
            }
            self.capture.poll();
            while self.capture.next_frame(&mut step) {
                self.handle(&mut step, looping);
            }
            if let Err(error) = self.capture.wait(bell) {
                tracing::warn!(%error, "cannot wait for the microphone");
                return;
            }
        }
    }

    fn handle(&mut self, step: &mut [f32; STEP_SAMPLES], looping: bool) {
        let processed = self.chain.process(step, true);
        if let Some(level) = self.meter.add(processed.level_db) {
            let _ = self.levels.unbounded_send(level);
        }
        if !looping {
            return;
        }
        let gain = if processed.open { self.volume } else { 0.0 };
        self.stereo.clear();
        for &sample in step.iter() {
            self.stereo.extend(std::iter::repeat_n(
                sample * gain,
                usize::from(OUTPUT_CHANNELS),
            ));
        }
        if self
            .playback
            .queued()
            .is_some_and(|queued| queued < LOOPBACK_QUEUE)
        {
            self.playback.write(&self.stereo);
        }
    }
}
