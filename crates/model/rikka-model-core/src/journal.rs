use std::sync::Arc;

use rikka_types::CancelToken;

use crate::error::{Error, Result};
use crate::progress::{Progress, Stage};
use crate::warning::Warning;

pub(crate) type Report = Arc<dyn Fn(Progress) + Send + Sync>;

pub(crate) struct Journal<'a> {
    cancel: &'a CancelToken,
    report: Option<&'a (dyn Fn(Progress) + Send + Sync)>,
    warnings: Vec<Warning>,
}

impl<'a> Journal<'a> {
    pub(crate) fn new(
        cancel: &'a CancelToken,
        report: Option<&'a (dyn Fn(Progress) + Send + Sync)>,
    ) -> Self {
        Self {
            cancel,
            report,
            warnings: Vec::new(),
        }
    }

    pub(crate) fn checkpoint(&self) -> Result<()> {
        match self.cancel.is_cancelled() {
            true => Err(Error::Cancelled),
            false => Ok(()),
        }
    }

    pub(crate) fn report(&self, stage: Stage, fraction: f32) {
        if let Some(report) = self.report {
            report(Progress::new(stage, fraction));
        }
    }

    pub(crate) fn warn(&mut self, warning: Warning) {
        tracing::debug!(%warning, "model warning");
        self.warnings.push(warning);
    }

    pub(crate) fn hold(&mut self, warnings: impl IntoIterator<Item = Warning>) {
        for warning in warnings {
            self.warn(warning);
        }
    }

    pub(crate) fn take(&mut self) -> Vec<Warning> {
        std::mem::take(&mut self.warnings)
    }
}
