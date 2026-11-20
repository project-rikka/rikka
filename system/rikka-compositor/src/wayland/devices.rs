use std::fs::OpenOptions;
use std::os::fd::OwnedFd;

use rikka_compositor_server::DisplayHandle;
use rikka_compositor_surface::{DmabufFeedback, DmabufTranche, Surfaces, TrancheFlags};
use rikka_compositor_types::{Format, FormatSet};
use rikka_renderer::Renderer;
use rustix::fs::{Dev, makedev};

use crate::color::{color_support, representation_support};
use crate::render::renderer_format_to_kms;
use crate::state::Compositor;

#[derive(Debug, Clone)]
pub(crate) struct Tranches {
    main: Dev,
    sampled: Vec<Format>,
}

impl Tranches {
    pub(crate) fn from_renderer(renderer: &Renderer) -> Option<Self> {
        let (main, _) = render_node(renderer)?;
        let sampled: Vec<Format> = renderer
            .gpu()
            .dmabuf_texture_formats()
            .iter()
            .map(renderer_format_to_kms)
            .collect();
        (!sampled.is_empty()).then_some(Self { main, sampled })
    }

    pub(crate) fn rendered(&self) -> Option<DmabufFeedback> {
        self.feedback(Vec::new())
    }

    pub(crate) fn scanned_out(&self, device: Dev, plane: &FormatSet) -> Option<DmabufFeedback> {
        let formats: Vec<Format> = plane
            .iter()
            .filter(|format| self.sampled.contains(format))
            .collect();
        let scanout = DmabufTranche {
            device,
            flags: TrancheFlags::SCANOUT | TrancheFlags::SAMPLING,
            formats,
        };
        self.feedback(vec![scanout])
    }

    fn feedback(&self, mut tranches: Vec<DmabufTranche>) -> Option<DmabufFeedback> {
        tranches.retain(|tranche| !tranche.formats.is_empty());
        tranches.push(DmabufTranche {
            device: self.main,
            flags: TrancheFlags::SAMPLING,
            formats: self.sampled.clone(),
        });
        DmabufFeedback::new(self.main, tranches)
            .inspect_err(|error| tracing::warn!("cannot offer dmabuf feedback: {error}"))
            .ok()
    }
}

pub(crate) fn render_node(renderer: &Renderer) -> Option<(Dev, String)> {
    let (major, minor) = renderer.gpu().drm_nodes()?.render?;
    Some((makedev(major, minor), format!("/dev/dri/renderD{minor}")))
}

pub(crate) fn primary_node(major: u32, minor: u32) -> Dev {
    makedev(major, minor)
}

pub(crate) fn enable(
    display: &DisplayHandle<Compositor>,
    surfaces: &mut Surfaces,
    renderer: &Renderer,
    syncobj: Option<OwnedFd>,
) {
    surfaces.enable_color_management(display, color_support());
    surfaces.set_representation_support(representation_support());
    match Tranches::from_renderer(renderer).and_then(|tranches| tranches.rendered()) {
        Some(feedback) => {
            surfaces.enable_dmabuf(display, feedback);
        }
        None => {
            tracing::info!("renderer cannot import dmabufs; clients fall back to shm")
        }
    }
    let device = match syncobj {
        Some(fd) => Ok(fd),
        None => match render_node(renderer) {
            Some((_, path)) => open(&path).map_err(|error| format!("{path}: {error}")),
            None => Err("renderer has no render node".to_owned()),
        },
    };
    let enabled = device.and_then(|fd| {
        surfaces
            .enable_syncobj(display, fd)
            .map_err(|error| error.to_string())
    });
    if let Err(error) = enabled {
        tracing::info!("explicit sync unavailable: {error}");
    }
}

fn open(path: &str) -> std::io::Result<OwnedFd> {
    Ok(OpenOptions::new().read(true).write(true).open(path)?.into())
}
