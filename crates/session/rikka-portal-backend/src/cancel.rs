use std::process::Child;
use std::sync::Mutex;

use futures_channel::oneshot;

#[derive(Debug)]
pub(crate) struct Cancel {
    running: Mutex<Running>,
}

#[derive(Debug)]
struct Running {
    stopped: bool,
    child: Option<Child>,
    wake: Option<oneshot::Sender<()>>,
}

impl Cancel {
    pub(crate) fn new() -> (Cancel, oneshot::Receiver<()>) {
        let (wake, stopped) = oneshot::channel();
        (
            Cancel {
                running: Mutex::new(Running {
                    stopped: false,
                    child: None,
                    wake: Some(wake),
                }),
            },
            stopped,
        )
    }

    pub(crate) fn attach(&self, child: Child) -> bool {
        let Ok(mut running) = self.running.lock() else {
            return false;
        };
        running.child = Some(child);
        if running.stopped {
            running.kill();
            return false;
        }
        true
    }

    pub(crate) fn stop(&self) {
        let Ok(mut running) = self.running.lock() else {
            return;
        };
        running.stopped = true;
        running.kill();
        if let Some(wake) = running.wake.take() {
            wake.send(()).ok();
        }
    }

    pub(crate) fn reap(&self) {
        let child = self
            .running
            .lock()
            .ok()
            .and_then(|mut running| running.child.take());
        if let Some(mut child) = child {
            child.wait().ok();
        }
    }
}

impl Running {
    fn kill(&mut self) {
        if let Some(child) = self.child.as_mut() {
            child.kill().ok();
        }
    }
}
