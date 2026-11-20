use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use rikka_seam::{Call, Dispatcher, Seam, SeamClient, SeamError};

const TICK: Duration = Duration::from_millis(10);

struct ChannelDispatcher(mpsc::Sender<Call>);

impl Dispatcher for ChannelDispatcher {
    fn enqueue(&self, call: Call) {
        if let Err(returned) = self.0.send(call) {
            returned.0.fail(SeamError::Disconnected);
        }
    }
}

#[derive(Debug)]
pub struct Host {
    client: SeamClient,
    stop: Option<mpsc::Sender<()>>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Host {
    #[must_use]
    pub fn start(register: impl FnOnce(&mut Seam) + Send + 'static) -> Self {
        let (calls, incoming) = mpsc::channel();
        let (stop, stopped) = mpsc::channel::<()>();
        let (ready, started) = mpsc::channel();

        let thread = thread::Builder::new()
            .name("rikka-seam-test-host".to_owned())
            .spawn(move || {
                let mut seam = Seam::new(ChannelDispatcher(calls));
                register(&mut seam);
                ready
                    .send(seam.client())
                    .expect("test should be waiting for the client");

                loop {
                    match incoming.recv_timeout(TICK) {
                        Ok(call) => seam.dispatch(&mut (), call),
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            if stopped.try_recv() == Err(mpsc::TryRecvError::Disconnected) {
                                return;
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => return,
                    }
                }
            })
            .expect("test seam thread should spawn");

        Self {
            client: started.recv().expect("test seam host should start"),
            stop: Some(stop),
            thread: Some(thread),
        }
    }

    #[must_use]
    pub fn client(&self) -> &SeamClient {
        &self.client
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        drop(self.stop.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
