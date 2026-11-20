use std::io::{PipeWriter, Write};
use std::os::fd::OwnedFd;
use std::sync::Arc;
use std::sync::mpsc::{Sender, channel};
use std::thread::JoinHandle;

use crate::core::frame::FrameTimeline;
use crate::error::{Error, Result};

pub(crate) struct Readiness {
    waits: Option<Sender<(u64, PipeWriter)>>,
    waiter: Option<JoinHandle<()>>,
}

impl Readiness {
    pub(crate) fn new(timeline: Arc<FrameTimeline>) -> Result<Self> {
        let (waits, queued) = channel::<(u64, PipeWriter)>();
        let waiter = std::thread::Builder::new()
            .name("rikka-scanout-wait".into())
            .spawn(move || {
                for (value, mut writer) in queued {
                    match timeline.wait(value) {
                        Ok(()) => {
                            if let Err(error) = writer.write_all(&[1]) {
                                tracing::debug!("scanout readiness pipe closed: {error}");
                            }
                        }
                        Err(error) => {
                            tracing::error!("failed to wait for scanout frame: {error}")
                        }
                    }
                }
            })
            .map_err(|error| Error::ScanoutWaiter(Box::new(error)))?;
        Ok(Self {
            waits: Some(waits),
            waiter: Some(waiter),
        })
    }

    pub(crate) fn ready_fd(&self, value: u64) -> Result<OwnedFd> {
        let (reader, writer) =
            std::io::pipe().map_err(|error| Error::ScanoutWaiter(Box::new(error)))?;
        self.waits
            .as_ref()
            .expect("scanout waiter outlives its owner")
            .send((value, writer))
            .map_err(|_| Error::ScanoutWaiter("scanout waiter thread exited".into()))?;
        Ok(reader.into())
    }
}

impl Drop for Readiness {
    fn drop(&mut self) {
        drop(self.waits.take());
        if let Some(waiter) = self.waiter.take()
            && waiter.join().is_err()
        {
            tracing::error!("scanout waiter thread panicked");
        }
    }
}

impl std::fmt::Debug for Readiness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Readiness").finish_non_exhaustive()
    }
}
