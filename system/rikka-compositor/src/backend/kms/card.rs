use std::fmt;

use rikka_compositor_kms::{Device, Gpu, Session};
use rikka_renderer::Renderer;

use super::connectors::Connectors;
use crate::programs::COMPOSITOR;

pub(super) type CardId = u64;

pub(super) struct Card {
    pub(super) device: Device,
    pub(super) connectors: Connectors,
    pub(super) renders: bool,
    copier: Copier,
}

enum Copier {
    Untried,
    Ready(Box<Renderer>),
    Unavailable,
}

impl Card {
    pub(super) fn new(device: Device, renders: bool) -> Self {
        Self {
            device,
            connectors: Connectors::default(),
            renders,
            copier: Copier::Untried,
        }
    }

    pub(super) fn id(&self) -> CardId {
        self.device.gpu().device_number()
    }

    pub(super) fn gpu(&self) -> &Gpu {
        self.device.gpu()
    }

    pub(super) fn copier(&mut self) -> Option<&mut Renderer> {
        if matches!(self.copier, Copier::Untried) {
            let gpu = self.device.gpu();
            self.copier = match rikka_gpu::Gpu::builder()
                .app_name(COMPOSITOR)
                .drm_node(gpu.major(), gpu.minor())
                .build()
                .and_then(|device| Renderer::new(&device))
            {
                Ok(renderer) => {
                    tracing::info!("{} uses a copy renderer", gpu.path().display());
                    Copier::Ready(Box::new(renderer))
                }
                Err(error) => {
                    tracing::info!(
                        "{}: cannot create copy renderer: {error}",
                        gpu.path().display()
                    );
                    Copier::Unavailable
                }
            };
        }
        self.copy_renderer()
    }

    pub(super) fn copy_renderer(&mut self) -> Option<&mut Renderer> {
        match &mut self.copier {
            Copier::Ready(renderer) => Some(renderer),
            Copier::Untried | Copier::Unavailable => None,
        }
    }

    pub(super) fn drawing(&mut self) -> (&Device, Option<&mut Renderer>) {
        let copier = match &mut self.copier {
            Copier::Ready(renderer) => Some(&mut **renderer),
            Copier::Untried | Copier::Unavailable => None,
        };
        (&self.device, copier)
    }

    pub(super) fn forget_copier(&mut self) {
        if matches!(self.copier, Copier::Ready(_)) {
            self.copier = Copier::Untried;
        }
    }

    pub(super) fn close(self, session: &Session) {
        let path = self.gpu().path().to_owned();
        drop(self.copier);
        if let Err(error) = self.device.close(session) {
            tracing::warn!("cannot close {}: {error}", path.display());
        }
    }
}

impl fmt::Debug for Card {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Card")
            .field("gpu", self.gpu())
            .field("renders", &self.renders)
            .field("copies", &matches!(self.copier, Copier::Ready(_)))
            .finish_non_exhaustive()
    }
}
