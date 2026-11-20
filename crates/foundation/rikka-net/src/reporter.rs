use std::time::{Duration, Instant};

use crate::cancel::{CancelGuard, CancelToken};
use crate::error::{Error, Result};
use crate::meter::Meter;
use crate::progress::ProgressCallback;

const REPORT_INTERVAL: Duration = Duration::from_millis(100);

pub(crate) struct Reporter {
    callback: Option<ProgressCallback>,
    cancel: Option<CancelToken>,
    meter: Meter,
    reported_at: Instant,
}

impl Reporter {
    pub(crate) fn new(
        total: Option<u64>,
        resumed: u64,
        callback: Option<ProgressCallback>,
        cancel: Option<CancelToken>,
    ) -> Reporter {
        Reporter {
            callback,
            cancel,
            meter: Meter::new(total, resumed),
            reported_at: Instant::now() - REPORT_INTERVAL,
        }
    }

    pub(crate) fn advance(&mut self, bytes: u64) -> Result<()> {
        self.meter.record(bytes);

        if self.reported_at.elapsed() < REPORT_INTERVAL {
            return self.check_cancelled();
        }
        self.report()
    }

    pub(crate) fn report(&mut self) -> Result<()> {
        self.check_cancelled()?;
        self.reported_at = Instant::now();

        let progress = self.meter.progress();
        let Some(callback) = self.callback.as_mut() else {
            return Ok(());
        };
        if callback(progress).is_cancel() {
            return Err(Error::Cancelled);
        }
        Ok(())
    }

    pub(crate) fn finish(&mut self) {
        let progress = self.meter.progress();
        if let Some(callback) = self.callback.as_mut() {
            let _ = callback(progress);
        }
    }

    pub(crate) fn set_total(&mut self, total: Option<u64>) {
        self.meter.set_total(total);
    }

    pub(crate) fn resumed_from(&mut self, position: u64) {
        self.meter.resumed_from(position);
    }

    fn check_cancelled(&self) -> Result<()> {
        match &self.cancel {
            Some(token) => token.guard(),
            None => Ok(()),
        }
    }
}

impl std::fmt::Debug for Reporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reporter")
            .field("progress", &self.meter.progress())
            .finish_non_exhaustive()
    }
}
