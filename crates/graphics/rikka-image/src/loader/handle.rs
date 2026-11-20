use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};

use rikka_sync::Mutex;

use crate::budget::Reservation;
use crate::error::{Error, Result};
use crate::raster::Image;

#[derive(Debug, Default)]
pub(crate) struct JobState {
    cancelled: AtomicBool,
    finished: AtomicBool,
}

impl JobState {
    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    pub(crate) fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub(crate) fn finish(&self) {
        self.finished.store(true, Ordering::Release);
    }

    fn is_finished(&self) -> bool {
        self.finished.load(Ordering::Acquire)
    }
}

#[derive(Debug)]
pub(crate) struct Decoded {
    pub(crate) image: Result<Image>,
    pub(crate) follows_scheme: bool,
    pub(crate) reservation: Reservation,
}

#[derive(Debug)]
pub struct LoadHandle {
    result: Receiver<Decoded>,
    state: Arc<JobState>,
    reservation: Mutex<Reservation>,
    taken: AtomicBool,
    follows_scheme: AtomicBool,
}

impl LoadHandle {
    pub(crate) fn new(result: Receiver<Decoded>, state: Arc<JobState>) -> LoadHandle {
        LoadHandle {
            result,
            state,
            reservation: Mutex::new(Reservation::empty()),
            taken: AtomicBool::new(false),
            follows_scheme: AtomicBool::new(false),
        }
    }

    pub fn try_take(&self) -> Option<Result<Image>> {
        match self.result.try_recv() {
            Ok(decoded) => {
                self.taken.store(true, Ordering::Relaxed);
                Some(self.keep(decoded))
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                (!self.taken.swap(true, Ordering::Relaxed)).then_some(Err(Error::LoaderStopped))
            }
        }
    }

    pub fn is_ready(&self) -> bool {
        self.state.is_finished()
    }

    #[must_use]
    pub fn follows_color_scheme(&self) -> bool {
        self.follows_scheme.load(Ordering::Relaxed)
    }

    pub fn wait(&self) -> Result<Image> {
        match self.result.recv() {
            Ok(decoded) => {
                self.taken.store(true, Ordering::Relaxed);
                self.keep(decoded)
            }
            Err(_) => Err(Error::LoaderStopped),
        }
    }

    fn keep(&self, decoded: Decoded) -> Result<Image> {
        *self.reservation.lock() = decoded.reservation;
        self.follows_scheme
            .store(decoded.follows_scheme, Ordering::Relaxed);
        decoded.image
    }

    pub fn cancel(&self) {
        self.state.cancel();
    }
}

impl Drop for LoadHandle {
    fn drop(&mut self) {
        self.state.cancel();
    }
}
