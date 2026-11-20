use std::collections::HashSet;

#[derive(Default)]
pub(crate) struct Gpu {
    opened: Option<(u64, rikka_gpu::Gpu)>,
    backoff: HashSet<u64>,
}

impl Gpu {
    pub(crate) fn device_for(&mut self, device: u64) -> Option<&rikka_gpu::Gpu> {
        if self.backoff.contains(&device) {
            return None;
        }
        let is_current = self
            .opened
            .as_ref()
            .is_some_and(|(serving, _)| *serving == device);
        if !is_current {
            let (major, minor) = (rustix::fs::major(device), rustix::fs::minor(device));
            let built = rikka_gpu::Gpu::builder()
                .app_name("rikka-replay")
                .drm_node(major, minor)
                .compute_support()
                .video_encode()
                .build();
            match built {
                Ok(opened) => {
                    tracing::info!("encoding on GPU {major}:{minor}");
                    self.opened = Some((device, opened));
                }
                Err(error) => {
                    tracing::warn!(
                        "GPU {major}:{minor} cannot encode, falling back to shm: {error}"
                    );
                    self.backoff.insert(device);
                    return None;
                }
            }
        }
        self.opened.as_ref().map(|(_, opened)| opened)
    }

    pub(crate) fn refuse(&mut self, device: u64) {
        self.backoff.insert(device);
    }

    #[must_use]
    pub(crate) fn any(&self) -> Option<&rikka_gpu::Gpu> {
        self.opened.as_ref().map(|(_, opened)| opened)
    }
}
