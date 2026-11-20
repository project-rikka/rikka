use std::fmt;
use std::panic;
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, sync_channel};
use std::thread::{self, JoinHandle};

use crate::{Error, Result, Snapshot, World};

const THREAD_NAME: &str = "rikka-physics-step";

struct Job {
    world: Box<World>,
    frame_dt: f64,
    snapshot: Snapshot,
}

struct Idle {
    world: Box<World>,
    spare: Snapshot,
}

pub struct AsyncWorld {
    idle: Option<Idle>,
    front: Snapshot,
    jobs: Option<SyncSender<Job>>,
    results: Receiver<Job>,
    thread: Option<JoinHandle<()>>,
}

impl fmt::Debug for AsyncWorld {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AsyncWorld")
            .field("stepping", &self.idle.is_none())
            .field("time", &self.front.time())
            .finish()
    }
}

impl AsyncWorld {
    pub fn new(world: World) -> Result<Self> {
        let (jobs, job_receiver) = sync_channel::<Job>(1);
        let (result_sender, results) = sync_channel::<Job>(1);
        let thread = thread::Builder::new()
            .name(THREAD_NAME.to_owned())
            .spawn(move || {
                for mut job in job_receiver {
                    job.world.advance(job.frame_dt);
                    job.world.capture(&mut job.snapshot);
                    if result_sender.send(job).is_err() {
                        return;
                    }
                }
            })
            .map_err(Error::Thread)?;
        let front = world.snapshot();
        Ok(Self {
            idle: Some(Idle {
                world: Box::new(world),
                spare: front.clone(),
            }),
            front,
            jobs: Some(jobs),
            results,
            thread: Some(thread),
        })
    }

    pub fn step(&mut self, frame_dt: f64) -> Result<()> {
        self.wait()?;
        let Idle { world, spare } = self.idle.take().ok_or(Error::SteppingStopped)?;
        let jobs = self.jobs.as_ref().ok_or(Error::SteppingStopped)?;
        jobs.send(Job {
            world,
            frame_dt,
            snapshot: spare,
        })
        .map_err(|_| Error::SteppingStopped)
    }

    pub fn poll(&mut self) -> Result<bool> {
        if self.idle.is_some() {
            return Ok(true);
        }
        match self.results.try_recv() {
            Ok(job) => {
                self.complete(job);
                Ok(true)
            }
            Err(TryRecvError::Empty) => Ok(false),
            Err(TryRecvError::Disconnected) => Err(Error::SteppingStopped),
        }
    }

    pub fn wait(&mut self) -> Result<()> {
        if self.idle.is_some() {
            return Ok(());
        }
        let job = self.results.recv().map_err(|_| Error::SteppingStopped)?;
        self.complete(job);
        Ok(())
    }

    pub fn snapshot(&self) -> &Snapshot {
        &self.front
    }

    pub fn world(&mut self) -> Result<&mut World> {
        self.wait()?;
        self.idle
            .as_mut()
            .map(|idle| &mut *idle.world)
            .ok_or(Error::SteppingStopped)
    }

    pub fn into_world(mut self) -> Result<World> {
        self.wait()?;
        self.idle
            .take()
            .map(|idle| *idle.world)
            .ok_or(Error::SteppingStopped)
    }

    fn complete(&mut self, job: Job) {
        self.idle = Some(Idle {
            world: job.world,
            spare: std::mem::replace(&mut self.front, job.snapshot),
        });
    }
}

impl Drop for AsyncWorld {
    fn drop(&mut self) {
        self.jobs = None;
        if let Some(thread) = self.thread.take()
            && let Err(payload) = thread.join()
            && !thread::panicking()
        {
            panic::resume_unwind(payload);
        }
    }
}
