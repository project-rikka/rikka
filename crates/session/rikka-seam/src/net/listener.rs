use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener as StdUnixListener;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures_util::StreamExt;
use rikka_worker::Worker;
use tokio::net::UnixListener;
use tokio::sync::{Semaphore, mpsc, oneshot};
use tokio::task::JoinSet;

use super::worker::{Hangup, runtime, started};
use crate::capability::CapabilityId;
use crate::client::SeamClient;
use crate::error::SeamError;
use crate::event::Delivery;
use crate::net::frame::{self, Frame, OUTGOING};
use crate::request::Request;

const DIRECTORY_MODE: u32 = 0o700;
const SOCKET_MODE: u32 = 0o600;

const IN_FLIGHT: usize = 64;

#[derive(Debug)]
pub struct SocketListener {
    path: PathBuf,
    worker: Option<Worker<Hangup>>,
}

impl SocketListener {
    pub fn serve(path: impl Into<PathBuf>, client: SeamClient) -> io::Result<Self> {
        let path = path.into();
        let listener = bind(&path)?;

        let (stop, stopped) = oneshot::channel();
        let worker = Worker::spawn("rikka-seam", Hangup::new(stop), move || {
            let runtime = runtime()?;
            let listener = {
                let _inside = runtime.enter();
                UnixListener::from_std(listener)?
            };
            Ok(move || {
                runtime.block_on(async move {
                    tokio::select! {
                        _ = stopped => {}
                        () = accept_forever(listener, client) => {}
                    }
                });
            })
        })
        .map_err(started)?;

        Ok(Self {
            path,
            worker: Some(worker),
        })
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for SocketListener {
    fn drop(&mut self) {
        drop(self.worker.take());
        if let Err(error) = remove_if_exists(&self.path) {
            tracing::warn!(%error, path = %self.path.display(), "failed to remove seam socket");
        }
    }
}

fn bind(path: &Path) -> io::Result<StdUnixListener> {
    if let Some(directory) = path.parent() {
        fs::create_dir_all(directory)?;
        fs::set_permissions(directory, fs::Permissions::from_mode(DIRECTORY_MODE))?;
    }

    let staging = staging_path(path);
    remove_if_exists(&staging)?;
    let listener = StdUnixListener::bind(&staging)?;
    fs::set_permissions(&staging, fs::Permissions::from_mode(SOCKET_MODE))?;
    listener.set_nonblocking(true)?;
    fs::rename(&staging, path)?;
    Ok(listener)
}

fn staging_path(path: &Path) -> PathBuf {
    let mut name = std::ffi::OsString::from(".");
    name.push(path.file_name().unwrap_or_default());
    name.push(".binding");
    path.with_file_name(name)
}

fn remove_if_exists(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

async fn accept_forever(listener: UnixListener, client: SeamClient) {
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                tokio::task::spawn(converse(stream, client.clone()));
            }
            Err(error) => tracing::warn!("failed to accept seam connection: {error}"),
        }
    }
}

async fn converse(stream: tokio::net::UnixStream, client: SeamClient) {
    let (mut reading, writing) = stream.into_split();
    let (outgoing, queued) = mpsc::channel::<Frame>(OUTGOING);
    let running = Arc::new(Semaphore::new(IN_FLIGHT));
    let writer = frame::writer(writing, queued);
    let mut announcing = JoinSet::new();

    loop {
        match frame::read(&mut reading).await {
            Ok(Some(frame)) => {
                if !handle(frame, &client, &outgoing, &running, &mut announcing) {
                    break;
                }
            }
            Ok(None) => break,
            Err(error) => {
                tracing::debug!("invalid frame from seam peer: {error}");
                break;
            }
        }
    }

    announcing.shutdown().await;
    drop(outgoing);
    if let Err(error) = writer.await {
        tracing::warn!(%error, "seam writer task failed");
    }
}

fn handle(
    frame: Frame,
    client: &SeamClient,
    outgoing: &mpsc::Sender<Frame>,
    running: &Arc<Semaphore>,
    announcing: &mut JoinSet<()>,
) -> bool {
    match frame {
        Frame::Call {
            id,
            capability,
            method,
            payload,
            require,
        } => {
            let Ok(permit) = Arc::clone(running).try_acquire_owned() else {
                let outgoing = outgoing.clone();
                tokio::task::spawn(async move {
                    let _ = outgoing.send(refusal(id, &SeamError::Overloaded)).await;
                });
                return true;
            };
            let mut request = Request::new(capability, method).params(payload);
            if let Some(version) = require {
                request = request.require(version);
            }
            let reply = client.call(request);
            let outgoing = outgoing.clone();
            tokio::task::spawn(async move {
                let frame = match reply.await {
                    Ok(payload) => Frame::Reply { id, payload },
                    Err(error) => refusal(id, &error),
                };
                let _ = outgoing.send(frame).await;
                drop(permit);
            });
            true
        }
        Frame::Subscribe { capability } => {
            announce(client, capability, outgoing.clone(), announcing);
            true
        }
        Frame::Reply { .. } | Frame::Fail { .. } | Frame::Event { .. } | Frame::Lagged { .. } => {
            tracing::debug!("unexpected reply from seam peer");
            false
        }
    }
}

fn refusal(id: u64, error: &SeamError) -> Frame {
    Frame::Fail {
        id,
        code: error.code(),
        message: error.to_string(),
    }
}

fn announce(
    client: &SeamClient,
    capability: Option<CapabilityId>,
    outgoing: mpsc::Sender<Frame>,
    announcing: &mut JoinSet<()>,
) {
    let mut events = match capability {
        Some(capability) => client.subscribe_to(capability),
        None => client.subscribe(),
    };
    announcing.spawn(async move {
        while let Some(delivery) = events.next().await {
            let frame = match delivery {
                Delivery::Event(event) => Frame::Event { event },
                Delivery::Lagged(lagged) => Frame::Lagged {
                    capabilities: lagged.capabilities().to_vec(),
                    count: lagged.count(),
                },
            };
            if outgoing.send(frame).await.is_err() {
                break;
            }
        }
    });
}
