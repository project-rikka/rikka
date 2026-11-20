use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::sync::{Arc, mpsc};

use pipewire::channel::{Receiver, Sender};
use pipewire::loop_::Timeout;
use rikka_pipewire::{Connection, LoopWorker};

use crate::config::EngineConfig;
use crate::error::{Error, Result};
use crate::host::{Chores, Shared, THREAD};
use crate::lost::DeviceLost;
use crate::mixer::{Mixer, Retired};
use crate::output::OutputPlan;
use crate::spawn;

use super::graph::Graph;
use super::output::{self, Output};

#[derive(Clone)]
pub(crate) struct Resume(Sender<()>);

impl Resume {
    pub(crate) fn send(&self) -> bool {
        self.0.send(()).is_ok()
    }
}

pub(crate) struct Resumes(Receiver<()>);

pub(crate) fn resume_channel() -> Result<(Resume, Resumes)> {
    let (resume, resumes) = pipewire::channel::channel();
    Ok((Resume(resume), Resumes(resumes)))
}

#[derive(Debug)]
pub(crate) struct OutputHost {
    _worker: LoopWorker,
}

impl OutputHost {
    pub(crate) fn start(
        config: EngineConfig,
        mix: impl FnOnce(&OutputPlan) -> Mixer + Send + 'static,
        shared: Shared,
        retired: rikka_ring::Consumer<Retired>,
        resumes: Resumes,
    ) -> Result<(OutputHost, OutputPlan)> {
        let (planned, plans) = mpsc::sync_channel(1);
        let worker = rikka_pipewire::spawn(THREAD, move |stopping| {
            let connection = Connection::open()?;
            let (target, plan) = output::plan(&Graph::read(&connection)?, &config)?;
            shared
                .latency
                .store(u64::from(plan.quantum), Ordering::Relaxed);
            let output = Output::connect(
                connection.core(),
                target,
                &plan,
                mix(&plan),
                Arc::clone(&shared.device_lost),
                shared.latency,
            )?;
            planned
                .send(plan)
                .map_err(|_| Error::ThreadDied { name: THREAD })?;
            Ok(move || {
                let chores = Chores::new(shared.activity, retired, shared.watched);
                run(
                    &connection,
                    &output,
                    chores,
                    &shared.device_lost,
                    resumes.0,
                    stopping,
                );
            })
        })
        .map_err(spawn::failed(THREAD))?;
        let plan = plans
            .recv()
            .map_err(|_| Error::ThreadDied { name: THREAD })?;
        Ok((OutputHost { _worker: worker }, plan))
    }
}

fn run(
    connection: &Connection,
    output: &Output,
    mut chores: Chores,
    device_lost: &Arc<DeviceLost>,
    resumes: Receiver<()>,
    stopping: Receiver<()>,
) {
    let events = connection.main_loop().loop_();
    let lost = Arc::clone(device_lost);
    let _core = connection
        .core()
        .add_listener_local()
        .error(move |id, _, _, message| {
            if id == pipewire::core::PW_ID_CORE {
                tracing::warn!("audio output lost its sound server: {message}");
                lost.mark();
            }
        })
        .register();
    let stopped = Rc::new(Cell::new(false));
    let stop = Rc::clone(&stopped);
    let _stopping = stopping.attach(events, move |()| stop.set(true));
    let resumed = Rc::new(Cell::new(false));
    let resume = Rc::clone(&resumed);
    let _resumes = resumes.attach(events, move |()| resume.set(true));

    while !stopped.get() {
        events.iterate(match chores.next_wake() {
            None => Timeout::Infinite,
            Some(wake) => Timeout::Finite(wake),
        });
        if resumed.take() {
            chores.resume(output);
        }
        chores.tend(output);
    }
}
