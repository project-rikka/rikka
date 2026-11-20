use std::os::fd::OwnedFd;
use std::sync::Arc;

use rikka_wayland_client::wl_data_source::WlDataSource;
use rikka_wayland_client::xdg_toplevel_drag_v1::XdgToplevelDragV1;

use crate::wayland::data_device::worker::End;
use rikka_types::DragAction;

pub(super) struct OutgoingDrag {
    pub(super) source: WlDataSource,
    pub(super) windows: Option<XdgToplevelDragV1>,
    entries: Vec<(String, Arc<[u8]>)>,
    action: Option<DragAction>,
    performed: bool,
}

impl OutgoingDrag {
    pub(super) fn new(
        source: WlDataSource,
        windows: Option<XdgToplevelDragV1>,
        entries: Vec<(String, Arc<[u8]>)>,
    ) -> Self {
        Self {
            source,
            windows,
            entries,
            action: None,
            performed: false,
        }
    }

    pub(super) fn send(&self, mime: &str, fd: OwnedFd) {
        match self.entries.iter().find(|(offered, _)| offered == mime) {
            Some((_, data)) => write_transfer(Arc::clone(data), fd),
            None => tracing::debug!("drag target requested unoffered type `{mime}`"),
        }
    }

    pub(super) fn chosen(&mut self, action: Option<DragAction>) {
        self.action = action;
    }

    pub(super) fn performed(&mut self) {
        self.performed = true;
    }

    pub(super) fn finished(&self) -> End {
        End::Landed(self.action.unwrap_or(DragAction::Copy))
    }

    pub(super) fn cancelled(&self) -> End {
        match self.performed {
            true => End::Nowhere,
            false => End::Aborted,
        }
    }

    pub(super) fn release(self) {
        self.source.destroy();
        if let Some(windows) = self.windows {
            windows.destroy();
        }
    }
}

fn write_transfer(data: Arc<[u8]>, fd: OwnedFd) {
    let spawned = std::thread::Builder::new()
        .name("rikka-drag-send".into())
        .spawn(move || {
            use std::io::Write as _;
            let mut file = std::fs::File::from(fd);
            if let Err(error) = file.write_all(&data) {
                tracing::debug!("drag transfer interrupted: {error}");
            }
        });
    if let Err(error) = spawned {
        tracing::warn!("failed to start drag transfer: {error}");
    }
}
