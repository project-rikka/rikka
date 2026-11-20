mod copied;
mod direct;
mod staged;

use rikka_compositor_kms::{Buffers, Connector, Dmabuf, DmabufPlane, Output, OutputConfig};
use rikka_compositor_types::{Format, FormatSet, Fourcc, Modifier};
use rikka_renderer::{DmabufTarget, ExportedDmabuf, Renderer, SharedRows, TargetFormat};

pub(super) use copied::Copied;
pub(super) use direct::Direct;
pub(super) use staged::Staged;

use super::card::Card;
use crate::render::renderer_format_to_kms;

const STAGED_BUFFERS: usize = 3;

const PREFERRED_FORMATS: [Fourcc; 2] = [Fourcc::XRGB8888, Fourcc::ARGB8888];

const DEEP_FORMATS: [Fourcc; 4] = [
    Fourcc::XRGB2101010,
    Fourcc::ARGB2101010,
    Fourcc::XBGR2101010,
    Fourcc::ABGR2101010,
];

const COPY_TARGET_FORMATS: [Fourcc; 4] = [
    Fourcc::XBGR8888,
    Fourcc::ABGR8888,
    Fourcc::XRGB8888,
    Fourcc::ARGB8888,
];

const DEEP_COPY_TARGET_FORMATS: [Fourcc; 4] = [
    Fourcc::XBGR2101010,
    Fourcc::ABGR2101010,
    Fourcc::XRGB2101010,
    Fourcc::ARGB2101010,
];

const ROWS_FORMATS: [Fourcc; 2] = [Fourcc::XBGR8888, Fourcc(SharedRows::FOURCC)];

const DEEP_ROWS_FORMATS: [Fourcc; 2] = [Fourcc::XBGR2101010, Fourcc::ABGR2101010];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Depth {
    Eight,
    Ten,
}

impl Depth {
    fn of(config: &OutputConfig) -> Self {
        match config.is_hdr() {
            true => Self::Ten,
            false => Self::Eight,
        }
    }

    pub(super) fn scanned(self) -> &'static [Fourcc] {
        match self {
            Self::Eight => &PREFERRED_FORMATS,
            Self::Ten => &DEEP_FORMATS,
        }
    }

    fn copy_targets(self) -> &'static [Fourcc] {
        match self {
            Self::Eight => &COPY_TARGET_FORMATS,
            Self::Ten => &DEEP_COPY_TARGET_FORMATS,
        }
    }

    fn rows(self) -> &'static [Fourcc] {
        match self {
            Self::Eight => &ROWS_FORMATS,
            Self::Ten => &DEEP_ROWS_FORMATS,
        }
    }

    pub(super) fn target(self) -> TargetFormat {
        match self {
            Self::Eight => TargetFormat::Rgba8,
            Self::Ten => TargetFormat::Rgb10a2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Tier {
    Native,
    Shared,
    Exported,
    Copied,
    Blitted,
    Cpu,
}

pub(super) enum Link {
    Direct(Direct),
    Copied(Box<Copied>),
    Staged(Box<Staged>),
}

pub(super) struct EnabledOutput {
    pub(super) output: Output,
    pub(super) link: Link,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum LinkError {
    #[error("{0}")]
    Display(#[from] rikka_compositor_kms::Error),
    #[error("{0}")]
    Renderer(#[from] rikka_renderer::Error),
    #[error("no linear format shared by renderer and display")]
    NoFormat,
    #[error("GPU cannot render")]
    NoCopier,
}

#[derive(Debug, Clone)]
pub(super) struct Formats {
    rendered: FormatSet,
    linear: FormatSet,
    exported: FormatSet,
}

impl Tier {
    pub(super) fn fallbacks(renders: bool, from: Tier) -> impl Iterator<Item = Tier> {
        let tiers: &[Tier] = match renders {
            true => &[Tier::Native],
            false => &[
                Tier::Shared,
                Tier::Exported,
                Tier::Copied,
                Tier::Blitted,
                Tier::Cpu,
            ],
        };
        tiers.iter().copied().filter(move |tier| *tier >= from)
    }

    pub(super) fn fallback(self) -> Option<Tier> {
        match self {
            Tier::Native | Tier::Cpu => None,
            Tier::Shared => Some(Tier::Exported),
            Tier::Exported => Some(Tier::Copied),
            Tier::Copied => Some(Tier::Blitted),
            Tier::Blitted => Some(Tier::Cpu),
        }
    }

    pub(super) fn describe(self) -> &'static str {
        match self {
            Tier::Native => "rendered on its own GPU",
            Tier::Shared => "rendered directly into its buffers by the render GPU",
            Tier::Exported => "scanning out render-GPU buffers in system memory",
            Tier::Copied => "copied on its own GPU from the render GPU",
            Tier::Blitted => "scanning out render-GPU copies in system memory",
            Tier::Cpu => "copied through the CPU from the render GPU",
        }
    }
}

impl Formats {
    pub(super) fn from_renderer(renderer: &Renderer) -> Self {
        let rendered: FormatSet = renderer
            .gpu()
            .dmabuf_render_formats()
            .iter()
            .map(renderer_format_to_kms)
            .collect();
        let linear = only_linear(&rendered);
        let exported = only_linear(
            &renderer
                .gpu()
                .dmabuf_export_formats()
                .iter()
                .map(renderer_format_to_kms)
                .collect(),
        );
        Self {
            rendered,
            linear,
            exported,
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.rendered.is_empty()
    }

    pub(super) fn rendered(&self) -> FormatSet {
        self.rendered.clone()
    }
}

impl Link {
    pub(super) fn release(self, card: &mut Card) {
        if let Link::Copied(copied) = self
            && let Some(copier) = card.copy_renderer()
        {
            copied.release(copier);
        }
    }
}

pub(super) fn enable(
    tier: Tier,
    card: &mut Card,
    connector: &Connector,
    drm_config: OutputConfig,
    renderer: &mut Renderer,
    formats: &Formats,
) -> Result<EnabledOutput, LinkError> {
    let depth = Depth::of(&drm_config);
    match tier {
        Tier::Native => {
            let output = card.device.enable_output(
                connector,
                drm_config,
                Buffers::Rendered(formats.rendered.clone()),
            )?;
            Ok(EnabledOutput {
                output,
                link: Link::Direct(Direct::default()),
            })
        }
        Tier::Shared => {
            let output = card.device.enable_output(
                connector,
                drm_config,
                Buffers::Shared(formats.linear.clone()),
            )?;
            match Direct::imported(&output, renderer) {
                Ok(direct) => Ok(EnabledOutput {
                    output,
                    link: Link::Direct(direct),
                }),
                Err(error) => {
                    disable(card, output);
                    Err(error)
                }
            }
        }
        Tier::Exported => {
            let plane = card.device.primary_formats(connector)?;
            let fourcc = depth
                .scanned()
                .iter()
                .copied()
                .find(|fourcc| {
                    scans_linear(&plane, *fourcc)
                        && formats
                            .exported
                            .contains(Format::new(*fourcc, Modifier::LINEAR))
                })
                .ok_or(LinkError::NoFormat)?;
            let mode = drm_config.mode();
            let size = (mode.width(), mode.height());
            let exported = (0..STAGED_BUFFERS)
                .map(|_| export(renderer, size, fourcc))
                .collect::<Result<Vec<_>, _>>()?;
            let (dmabufs, targets): (Vec<Dmabuf>, Vec<DmabufTarget>) = exported.into_iter().unzip();
            let output =
                card.device
                    .enable_output(connector, drm_config, Buffers::Imported(dmabufs))?;
            Ok(EnabledOutput {
                link: Link::Direct(Direct::exported(&output, targets)),
                output,
            })
        }
        Tier::Copied => {
            let copier = card.copier().ok_or(LinkError::NoCopier)?;
            let fourcc =
                Copied::fourcc(copier, &formats.exported, depth).ok_or(LinkError::NoFormat)?;
            let drawable = Formats::from_renderer(copier).rendered;
            let output =
                card.device
                    .enable_output(connector, drm_config, Buffers::Rendered(drawable))?;
            let copier = card.copy_renderer().ok_or(LinkError::NoCopier)?;
            match Copied::new(&output, copier, renderer, fourcc) {
                Ok(copied) => Ok(EnabledOutput {
                    output,
                    link: Link::Copied(Box::new(copied)),
                }),
                Err(error) => {
                    disable(card, output);
                    Err(error)
                }
            }
        }
        Tier::Blitted => {
            let plane = card.device.primary_formats(connector)?;
            let fourcc = depth
                .rows()
                .iter()
                .copied()
                .find(|fourcc| scans_linear(&plane, *fourcc))
                .ok_or(LinkError::NoFormat)?;
            let mode = drm_config.mode();
            let rows = (0..STAGED_BUFFERS)
                .map(|_| renderer.export_rows(mode.width(), mode.height()))
                .collect::<Result<Vec<_>, _>>()?;
            let dmabufs = rows
                .iter()
                .map(|shared| shown_rows(shared, fourcc))
                .collect::<Result<Vec<_>, _>>()?;
            let output =
                card.device
                    .enable_output(connector, drm_config, Buffers::Imported(dmabufs))?;
            staged(card, output, |output| {
                Staged::rows(output, renderer, rows, depth.target())
            })
        }
        Tier::Cpu => {
            let output = card.device.enable_output(
                connector,
                drm_config,
                Buffers::Mapped(depth.copy_targets().to_vec()),
            )?;
            staged(card, output, |output| {
                Staged::mapped(output, renderer, depth.target())
            })
        }
    }
}

fn staged(
    card: &mut Card,
    output: Output,
    stage: impl FnOnce(&Output) -> Result<Staged, LinkError>,
) -> Result<EnabledOutput, LinkError> {
    match stage(&output) {
        Ok(staged) => Ok(EnabledOutput {
            output,
            link: Link::Staged(Box::new(staged)),
        }),
        Err(error) => {
            disable(card, output);
            Err(error)
        }
    }
}

fn shown_rows(shared: &SharedRows, fourcc: Fourcc) -> Result<Dmabuf, LinkError> {
    Ok(Dmabuf::new(
        shared.width,
        shared.height,
        Format::new(fourcc, Modifier::LINEAR),
        vec![DmabufPlane {
            fd: shared
                .fd
                .try_clone()
                .map_err(rikka_compositor_kms::Error::Export)?,
            offset: 0,
            stride: shared.stride,
        }],
    ))
}

fn disable(card: &mut Card, output: Output) {
    if let Err(error) = card.device.disable_output(output) {
        tracing::debug!("cannot disable output after failed modeset: {error}");
    }
}

fn export(
    renderer: &mut Renderer,
    (width, height): (u32, u32),
    fourcc: Fourcc,
) -> Result<(Dmabuf, DmabufTarget), LinkError> {
    let exported = renderer.export_shared_dmabuf(width, height, fourcc.0, &[Modifier::LINEAR.0])?;
    let dmabuf = dmabuf_of(&exported, width, height)?;
    Ok((dmabuf, exported.target))
}

fn dmabuf_of(exported: &ExportedDmabuf, width: u32, height: u32) -> Result<Dmabuf, LinkError> {
    let planes = exported
        .planes
        .iter()
        .map(|plane| {
            Ok(DmabufPlane {
                fd: plane
                    .fd
                    .try_clone()
                    .map_err(rikka_compositor_kms::Error::Export)?,
                offset: plane.offset,
                stride: plane.stride,
            })
        })
        .collect::<Result<Vec<_>, LinkError>>()?;
    Ok(Dmabuf::new(
        width,
        height,
        renderer_format_to_kms(&exported.format),
        planes,
    ))
}

fn only_linear(formats: &FormatSet) -> FormatSet {
    formats
        .iter()
        .filter(|format| format.modifier == Modifier::LINEAR)
        .collect()
}

fn scans_linear(plane: &FormatSet, fourcc: Fourcc) -> bool {
    plane.contains(Format::new(fourcc, Modifier::LINEAR))
        || plane.contains(Format::new(fourcc, Modifier::INVALID))
}
