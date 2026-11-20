use std::os::windows::io::AsHandle;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};

use rikka_worker::{EventFd, EventNotifier, Stop, Worker, event_fd};
use windows::Win32::Media::Audio::eRender;

use crate::config::EngineConfig;
use crate::error::{Error, Result};
use crate::host::{Chores, Shared, THREAD};
use crate::lost::DeviceLost;
use crate::mixer::{Mixer, Retired};
use crate::output::OutputPlan;
use crate::spawn;

use super::com::Apartment;
use super::endpoint::{Endpoint, Endpoints};
use super::notify;
use super::output::{Output, Signals};

#[derive(Clone)]
pub(crate) struct Resume(EventNotifier);

impl Resume {
    pub(crate) fn send(&self) -> bool {
        match self.0.notify() {
            Ok(()) => true,
            Err(error) => {
                tracing::warn!(%error, "failed to resume the audio output");
                false
            }
        }
    }
}

pub(crate) struct Resumes(EventFd);

pub(crate) fn resume_channel() -> Result<(Resume, Resumes)> {
    let (resume, resumes) = event_fd()?;
    Ok((Resume(resume), Resumes(resumes)))
}

pub(super) struct Stopper {
    stopping: Arc<AtomicBool>,
    bell: EventNotifier,
}

impl Stopper {
    pub(super) fn new() -> Result<(Stopper, Arc<AtomicBool>, EventFd)> {
        let (bell, stopped) = event_fd()?;
        let stopping = Arc::new(AtomicBool::new(false));
        Ok((
            Stopper {
                stopping: Arc::clone(&stopping),
                bell,
            },
            stopping,
            stopped,
        ))
    }
}

impl Stop for Stopper {
    fn stop(&self) {
        self.stopping.store(true, Ordering::Release);
        if let Err(error) = self.bell.notify() {
            tracing::warn!(%error, "failed to stop an audio thread");
        }
    }
}

#[derive(Debug)]
pub(crate) struct OutputHost {
    _worker: Worker<Stopper>,
}

impl std::fmt::Debug for Stopper {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Stopper").finish_non_exhaustive()
    }
}

enum Route {
    Default,
    Named(String),
}

impl Route {
    fn resolve(&self, endpoints: &Endpoints) -> Result<Endpoint> {
        match self {
            Route::Default => endpoints.default(eRender)?.ok_or(Error::NoOutputDevice),
            Route::Named(name) => endpoints
                .find(eRender, name)?
                .ok_or_else(|| Error::DeviceNotFound(name.clone())),
        }
    }
}

impl OutputHost {
    pub(crate) fn start(
        config: EngineConfig,
        mix: impl FnOnce(&OutputPlan) -> Mixer + Send + 'static,
        shared: Shared,
        retired: rikka_ring::Consumer<Retired>,
        resumes: Resumes,
    ) -> Result<(OutputHost, OutputPlan)> {
        let (stopper, stopping, stopped) = Stopper::new()?;
        let (planned, plans) = mpsc::sync_channel(1);
        let worker = Worker::spawn(THREAD, stopper, move || {
            let apartment = Apartment::enter()?;
            let endpoints = Endpoints::new()?;
            let (changed_bell, changed) = event_fd()?;
            let registration = endpoints.register(notify::listener(changed_bell))?;
            let route = match &config.device_name {
                None => Route::Default,
                Some(name) => Route::Named(name.clone()),
            };
            let endpoint = route.resolve(&endpoints)?;
            let plan = OutputPlan::new(&config, Some(Output::native_rate(&endpoint)?))?;
            shared
                .latency
                .store(u64::from(plan.quantum), Ordering::Relaxed);
            let (failed_bell, failed) = event_fd()?;
            let signals = Signals {
                failed: failed_bell,
                latency: Arc::clone(&shared.latency),
            };
            let output = Output::open(&endpoint, &plan, mix(&plan), &signals)?;
            planned
                .send(plan.clone())
                .map_err(|_| Error::ThreadDied { name: THREAD })?;
            Ok(move || {
                let hosting = Hosting {
                    endpoints,
                    route,
                    plan,
                    signals,
                    device_lost: shared.device_lost,
                    chores: Chores::new(shared.activity, retired, shared.watched),
                };
                hosting.run(
                    output,
                    Wakes {
                        stopping,
                        stopped,
                        resumes: resumes.0,
                        changed,
                        failed,
                    },
                );
                drop(registration);
                drop(apartment);
            })
        })
        .map_err(spawn::failed(THREAD))?;
        let plan = plans
            .recv()
            .map_err(|_| Error::ThreadDied { name: THREAD })?;
        Ok((OutputHost { _worker: worker }, plan))
    }
}

struct Wakes {
    stopping: Arc<AtomicBool>,
    stopped: EventFd,
    resumes: EventFd,
    changed: EventFd,
    failed: EventFd,
}

struct Hosting {
    endpoints: Endpoints,
    route: Route,
    plan: OutputPlan,
    signals: Signals,
    device_lost: Arc<DeviceLost>,
    chores: Chores,
}

impl Hosting {
    fn run(mut self, mut output: Output, wakes: Wakes) {
        loop {
            let waited = wakes.stopped.wait_signaled(
                [
                    wakes.resumes.as_handle(),
                    wakes.changed.as_handle(),
                    wakes.failed.as_handle(),
                ],
                self.chores.next_wake(),
            );
            if wakes.stopping.load(Ordering::Acquire) {
                return;
            }
            let [resumed, changed, failed] = match waited {
                Ok(signaled) => signaled,
                Err(error) => {
                    tracing::warn!(%error, "audio output host cannot wait");
                    return;
                }
            };
            if resumed {
                wakes.resumes.drain();
                self.chores.resume(&output);
            }
            if changed || failed {
                wakes.changed.drain();
                wakes.failed.drain();
                output = match self.reroute(output) {
                    Some(output) => output,
                    None => {
                        self.device_lost.mark();
                        return wait_for_stop(&wakes);
                    }
                };
            }
            self.chores.tend(&output);
        }
    }

    fn reroute(&mut self, output: Output) -> Option<Output> {
        let target = match &self.route {
            Route::Default => match self.endpoints.default(eRender) {
                Ok(Some(endpoint)) => endpoint,
                Ok(None) => {
                    tracing::warn!("no audio output device is left");
                    return None;
                }
                Err(error) => {
                    tracing::warn!(%error, "failed to find the default audio output");
                    return None;
                }
            },
            Route::Named(name) => match self.route.resolve(&self.endpoints) {
                Ok(endpoint) => endpoint,
                Err(error) => {
                    tracing::warn!(%error, name, "audio output device is gone");
                    return None;
                }
            },
        };
        if target.id == output.id() && !output.has_failed() {
            return Some(output);
        }
        let suspended = self.chores.is_suspended();
        let mixer = output.close()?;
        match Output::open(&target, &self.plan, mixer, &self.signals) {
            Ok(output) => {
                tracing::info!(device = target.name, "audio output moved");
                if suspended && let Err(error) = output.set_active(false) {
                    tracing::warn!(%error, "failed to keep the moved audio output suspended");
                }
                Some(output)
            }
            Err(error) => {
                tracing::warn!(%error, device = target.name, "failed to reopen the audio output");
                None
            }
        }
    }
}

fn wait_for_stop(wakes: &Wakes) {
    while !wakes.stopping.load(Ordering::Acquire) {
        if let Err(error) = wakes.stopped.wait() {
            tracing::warn!(%error, "audio output host cannot wait");
            return;
        }
        wakes.stopped.drain();
    }
}
