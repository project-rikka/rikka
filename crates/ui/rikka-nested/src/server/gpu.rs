use std::fs::OpenOptions;
use std::io;
use std::os::fd::OwnedFd;
use std::os::unix::fs::FileTypeExt;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use rikka_compositor_server::DisplayHandle;
use rikka_compositor_surface::{DmabufFeedback, DmabufTranche, Surfaces, TrancheFlags};
use rikka_compositor_types::{Format, Fourcc, Modifier};
use rikka_picture::SharedGpu;
use rustix::fs::{Dev, makedev};

use super::state::State;
use crate::command::{Command, Link};

const DRI: &str = "/dev/dri";

pub(crate) fn await_published(link: Link, patience: Duration) -> io::Result<()> {
    std::thread::Builder::new()
        .name("rikka-nested-gpu".into())
        .spawn(move || {
            let gpu = SharedGpu::wait(patience).map(Arc::unwrap_or_clone);
            let _ = link.send(Command::Gpu(gpu));
        })
        .map(drop)
}

impl State {
    pub(crate) fn settle_gpu(&mut self, gpu: Option<SharedGpu>) {
        match gpu {
            Some(gpu) => enable(&self.display, &mut self.surfaces, &gpu),
            None => tracing::info!("no GPU published, nested clients fall back to shm"),
        }
        self.gpu_settled = true;
    }
}

fn enable(display: &DisplayHandle<State>, surfaces: &mut Surfaces, gpu: &SharedGpu) {
    let Some((major, minor)) = gpu.render_node else {
        tracing::info!("GPU has no render node, nested clients fall back to shm");
        return;
    };
    let device = makedev(major, minor);
    let formats: Vec<Format> = gpu
        .texture_formats
        .iter()
        .map(|format| Format::new(Fourcc(format.fourcc), Modifier(format.modifier)))
        .collect();
    if formats.is_empty() {
        tracing::info!("GPU samples no dmabuf formats, nested clients fall back to shm");
        return;
    }
    let tranche = DmabufTranche {
        device,
        flags: TrancheFlags::SAMPLING,
        formats,
    };
    match DmabufFeedback::new(device, vec![tranche]) {
        Ok(feedback) => {
            surfaces.enable_dmabuf(display, feedback);
        }
        Err(error) => {
            tracing::warn!("failed to enable linux-dmabuf for nested clients: {error}");
            return;
        }
    }
    let syncobj = node_path(device)
        .ok_or_else(|| "render node missing from /dev/dri".to_owned())
        .and_then(|path| open(&path).map_err(|error| format!("{}: {error}", path.display())))
        .and_then(|fd| {
            surfaces
                .enable_syncobj(display, fd)
                .map_err(|error| error.to_string())
        });
    if let Err(error) = syncobj {
        tracing::info!("failed to enable explicit sync for nested clients: {error}");
    }
}

fn node_path(device: Dev) -> Option<PathBuf> {
    std::fs::read_dir(DRI)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            std::fs::metadata(path).is_ok_and(|metadata| {
                metadata.file_type().is_char_device() && metadata.rdev() == device
            })
        })
}

fn open(path: &PathBuf) -> std::io::Result<OwnedFd> {
    Ok(OpenOptions::new().read(true).write(true).open(path)?.into())
}
