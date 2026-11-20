use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

pub struct Listening {
    path: PathBuf,
    stopped: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
    _directory: tempfile::TempDir,
}

impl Listening {
    pub(crate) fn start(name: &str, mut accepted: impl FnMut(UnixStream) + Send + 'static) -> Self {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("bench.sock");
        let listener = UnixListener::bind(&path).expect("socket should bind");
        let stopped = Arc::new(AtomicBool::new(false));
        let stopping = Arc::clone(&stopped);
        let thread = thread::Builder::new()
            .name(format!("rikka-bench-{name}"))
            .spawn(move || {
                for stream in listener.incoming() {
                    if stopping.load(Ordering::Acquire) {
                        return;
                    }
                    accepted(stream.expect("listener should accept"));
                }
            })
            .expect("thread for the listener");
        Self {
            path,
            stopped,
            thread: Some(thread),
            _directory: directory,
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Listening {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        let _ = UnixStream::connect(&self.path);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl std::fmt::Debug for Listening {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Listening")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}
