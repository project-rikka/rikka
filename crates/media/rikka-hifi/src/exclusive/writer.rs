use std::os::fd::BorrowedFd;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use alsa::pcm::{IO, PCM, State};
use alsa::poll::Descriptors as _;
use rikka_worker::{EventFd, Stop, Worker};

use crate::exclusive::pcm::PcmDevice;
use crate::exclusive::realtime;
use crate::notice::{Notice, Notifier};
use crate::ring::{Ring, RingReader};

pub(crate) const NAME: &str = "rikka-hifi-alsa";

const RESUME_ATTEMPTS: usize = 50;
const RESUME_RETRY_INTERVAL: Duration = Duration::from_millis(20);

pub(crate) struct Control {
    stop: AtomicBool,
    paused: AtomicBool,
    ring: Arc<Ring>,
}

impl Control {
    pub(crate) fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::Release);
        self.ring.wake_consumer();
    }
}

impl Stop for Control {
    fn stop(&self) {
        self.stop.store(true, Ordering::Release);
        self.ring.wake_consumer();
    }
}

pub(crate) fn spawn(
    pcm_device: PcmDevice,
    reader: RingReader,
    filled: EventFd,
    notifier: Notifier,
) -> Result<Worker<Arc<Control>>, String> {
    let control = Arc::new(Control {
        stop: AtomicBool::new(false),
        paused: AtomicBool::new(false),
        ring: Arc::clone(reader.ring()),
    });
    let writing = Arc::clone(&control);
    Worker::spawn(NAME, control, move || {
        Ok::<_, std::convert::Infallible>(move || {
            realtime::request_realtime();
            Writer {
                pcm_device,
                control: writing,
                reader,
                filled,
                notifier,
            }
            .run();
        })
    })
    .map_err(|error| error.to_string())
}

struct Writer {
    pcm_device: PcmDevice,
    control: Arc<Control>,
    reader: RingReader,
    filled: EventFd,
    notifier: Notifier,
}

enum Interrupted {
    Lost(alsa::Error),
    Stopped,
}

impl Writer {
    fn run(mut self) {
        let pcm = &self.pcm_device.pcm;
        let io = pcm.io_bytes();
        let ring = Arc::clone(&self.control.ring);
        let mut chunk = vec![0; self.pcm_device.period * ring.frame()];
        let mut discard_epoch = ring.discards();
        let mut paused = false;
        loop {
            if self.control.stop.load(Ordering::Acquire) {
                stop(pcm);
                return;
            }
            if state(pcm) == State::Disconnected {
                self.notifier
                    .send(Notice::Lost("device disconnected".to_owned()));
                return;
            }
            let pause_requested = self.control.paused.load(Ordering::Acquire);
            if pause_requested != paused {
                paused = pause_requested;
                self.pause(pcm, paused);
            }
            if paused {
                self.wait_for_resume(pcm);
                continue;
            }
            if ring.discards() != discard_epoch {
                discard_epoch = ring.discards();
                restart(pcm);
            }
            let pulled = self.reader.pull(&mut chunk);
            if pulled == 0 {
                if state(pcm) == State::Prepared
                    && pcm.avail().is_ok_and(|room| room < self.pcm_device.buffer)
                    && let Err(error) = pcm.start()
                {
                    tracing::debug!(%error, "failed to start PCM");
                }
                self.measure(pcm);
                self.wait(self.period());
                continue;
            }
            match self.write(pcm, &io, &chunk[..pulled], discard_epoch) {
                Ok(()) => self.measure(pcm),
                Err(Interrupted::Stopped) => {}
                Err(Interrupted::Lost(error)) => {
                    self.notifier.send(Notice::Lost(error.to_string()));
                    return;
                }
            }
        }
    }

    fn write(
        &self,
        pcm: &PCM,
        io: &IO<'_, u8>,
        bytes: &[u8],
        discard_epoch: u64,
    ) -> Result<(), Interrupted> {
        let frame = self.control.ring.frame();
        let mut rest = bytes;
        while !rest.is_empty() {
            if self.control.stop.load(Ordering::Acquire)
                || self.control.paused.load(Ordering::Acquire)
                || self.control.ring.discards() != discard_epoch
            {
                return Err(Interrupted::Stopped);
            }
            match io.writei(rest) {
                Ok(frames) => rest = &rest[(frames * frame).min(rest.len())..],
                Err(error) => match error.errno() {
                    libc::EAGAIN => {
                        if let Err(error) = pcm.wait(Some(wait_millis(self.period() * 4))) {
                            recover(pcm, error)?;
                        }
                    }
                    _ => recover(pcm, error)?,
                },
            }
        }
        Ok(())
    }

    fn pause(&self, pcm: &PCM, paused: bool) {
        if self.pcm_device.can_pause {
            let switched = match (paused, state(pcm)) {
                (true, State::Running) => pcm.pause(true),
                (false, State::Paused) => pcm.pause(false),
                _ => Ok(()),
            };
            if let Err(error) = switched {
                tracing::debug!(%error, paused, "failed to pause PCM, restarting it");
                restart(pcm);
            }
        } else if paused {
            restart(pcm);
        }
    }

    fn measure(&self, pcm: &PCM) {
        let delay = match state(pcm) {
            State::Running | State::Paused | State::Prepared => pcm.delay().unwrap_or(0),
            _ => 0,
        };
        self.reader.set_latency(u64::try_from(delay).unwrap_or(0));
    }

    fn period(&self) -> Duration {
        self.pcm_device.period_time
    }

    fn wait(&self, timeout: Duration) {
        if let Err(error) = self.filled.wait_timeout(timeout) {
            tracing::debug!(%error, "failed to wait for audio data");
        }
        self.filled.drain();
    }

    fn wait_for_resume(&self, pcm: &PCM) {
        if !matches!(state(pcm), State::Paused | State::Prepared) {
            restart(pcm);
        }
        let descriptor = match pcm.get() {
            Ok(descriptors) => descriptors.first().map(|descriptor| descriptor.fd),
            Err(error) => {
                tracing::debug!(%error, "failed to read PCM poll descriptors");
                None
            }
        };
        let waited = match descriptor {
            Some(fd) => {
                let device = unsafe { BorrowedFd::borrow_raw(fd) };
                self.filled.wait_readable(device, None).map(drop)
            }
            None => self.filled.wait(),
        };
        if let Err(error) = waited {
            tracing::debug!(%error, "failed to wait while paused");
        }
        self.filled.drain();
    }
}

fn state(pcm: &PCM) -> State {
    let raw = i64::from(pcm.state_raw());
    State::all()
        .iter()
        .copied()
        .find(|state| *state as i64 == raw)
        .unwrap_or(State::Disconnected)
}

fn stop(pcm: &PCM) {
    if let Err(error) = pcm.drop() {
        tracing::debug!(%error, "failed to stop PCM");
    }
}

fn restart(pcm: &PCM) {
    stop(pcm);
    if let Err(error) = pcm.prepare() {
        tracing::debug!(%error, "failed to prepare PCM");
    }
}

fn recover(pcm: &PCM, error: alsa::Error) -> Result<(), Interrupted> {
    match error.errno() {
        libc::EPIPE => pcm.prepare().map_err(Interrupted::Lost),
        libc::ESTRPIPE => {
            for _ in 0..RESUME_ATTEMPTS {
                match pcm.resume() {
                    Ok(()) => return Ok(()),
                    Err(again) if again.errno() == libc::EAGAIN => {
                        std::thread::sleep(RESUME_RETRY_INTERVAL);
                    }
                    Err(_) => break,
                }
            }
            pcm.prepare().map_err(Interrupted::Lost)
        }
        _ => Err(Interrupted::Lost(error)),
    }
}

fn wait_millis(patience: Duration) -> u32 {
    u32::try_from(patience.as_millis())
        .unwrap_or(u32::MAX)
        .max(1)
}
