use std::sync::Arc;

use rikka_sync::{Mutex, MutexGuard};
use rikka_xdg::Catalog;
use shell_apps::Installed;
use shell_kit::StateFile;

use crate::usage::now;

const FILE: &str = "launcher-arrivals.json";

#[derive(Clone, Debug)]
pub(super) struct Arrivals(Arc<Mutex<Record>>);

#[derive(Debug, Default)]
struct Record {
    arrivals: app_arrivals::Arrivals,
    file: Option<StateFile>,
}

impl Arrivals {
    #[must_use]
    pub(super) fn follow(installed: &Installed) -> Arrivals {
        let arrivals = Arrivals(Arc::new(Mutex::new(Record::read())));
        installed.on_change({
            let arrivals = arrivals.clone();
            move |catalogue| arrivals.observe(catalogue)
        });
        arrivals.observe(&installed.current());
        arrivals
    }

    pub(super) fn read<R>(&self, read: impl FnOnce(&app_arrivals::Arrivals) -> R) -> R {
        read(&self.record().arrivals)
    }

    pub(super) fn observe(&self, catalogue: &Catalog) {
        let mut record = self.record();
        let present = catalogue.listable().map(|entry| entry.id.as_str());
        if !record.arrivals.observe(present, now()) {
            return;
        }
        record.write();
    }

    fn record(&self) -> MutexGuard<'_, Record> {
        self.0.lock()
    }
}

impl Record {
    fn read() -> Record {
        let Some(file) = StateFile::state(FILE) else {
            return Record::default();
        };
        Record {
            arrivals: file.load_json(),
            file: Some(file),
        }
    }

    fn write(&self) {
        let Some(file) = &self.file else {
            return;
        };
        match serde_json::to_string(&self.arrivals) {
            Ok(document) => file.save(document),
            Err(error) => tracing::warn!("failed to encode app arrivals: {error}"),
        }
    }
}

impl Drop for Record {
    fn drop(&mut self) {
        if let Some(file) = &self.file {
            file.flush();
        }
    }
}
