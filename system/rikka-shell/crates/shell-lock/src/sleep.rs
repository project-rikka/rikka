use std::rc::Rc;
use std::sync::Arc;

use rikka_power::{Delay, Sleep, SleepWatch};
use rikka_seam::{Request, SeamClient};
use rikka_sync::Mutex;

use crate::service::{CAPABILITY, SLEEP};

const WHO: &str = "Rikka";
const WHY: &str = "The screen locks before the computer sleeps";

#[derive(Clone, Debug)]
pub(super) struct SleepInhibitor {
    delay: Arc<Mutex<Option<Delay>>>,
    _watch: Option<Rc<SleepWatch>>,
}

impl SleepInhibitor {
    #[must_use]
    pub(crate) fn watch(seam: SeamClient) -> SleepInhibitor {
        let delay = Arc::new(Mutex::new(None));
        let held = Arc::clone(&delay);
        let watch = SleepWatch::start(WHO, WHY, move |sleep| {
            let mut guard = held.lock();
            match sleep {
                Sleep::Going(going) => {
                    *guard = Some(going);
                    drop(guard);
                    seam.notify(Request::new(CAPABILITY, SLEEP));
                }
                Sleep::Woke => *guard = None,
            }
        })
        .inspect_err(|error| tracing::warn!("failed to watch PrepareForSleep: {error}"))
        .ok();
        SleepInhibitor {
            delay,
            _watch: watch.map(Rc::new),
        }
    }

    pub(crate) fn release(&self) {
        let guard = self.delay.lock().take();
        if let Some(delay) = guard {
            delay.release();
        }
    }
}
