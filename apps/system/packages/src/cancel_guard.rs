use nixpkgs_search::{Cancel, Error, Result};

#[derive(Debug, Default)]
pub(crate) struct CancelGuard(Option<Cancel>);

impl CancelGuard {
    pub(crate) fn renew(&mut self) -> Cancel {
        self.stop();
        let cancel = Cancel::new();
        self.0 = Some(cancel.clone());
        cancel
    }

    pub(crate) fn stop(&mut self) {
        if let Some(cancel) = self.0.take() {
            cancel.stop();
        }
    }
}

impl Drop for CancelGuard {
    fn drop(&mut self) {
        self.stop();
    }
}

pub(crate) fn unless_cancelled<T>(result: Result<T>) -> Option<Result<T>> {
    match result {
        Err(Error::Cancelled) => None,
        result => Some(result),
    }
}
