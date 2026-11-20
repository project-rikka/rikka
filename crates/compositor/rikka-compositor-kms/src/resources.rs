use drm::control::{Device as _, crtc, plane};
use drm::{ClientCapability, Device as _, DriverCapability};
use rikka_compositor_types::{Format, FormatSet, Fourcc, Modifier};

use crate::claim::{PlaneFacts, PlaneKind, Stacking};
use crate::drm_fd::DrmFd;
use crate::error::{Error, Result};
use crate::format::parse_in_formats;
use crate::properties::{CrtcProps, ObjectProperties, PlaneProps};

const DEFAULT_CURSOR: u64 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Caps {
    pub(crate) explicit_modifiers: bool,
    pub(crate) monotonic_timestamps: bool,
    pub(crate) async_flips: bool,
    pub(crate) cursor: (u32, u32),
}

#[derive(Debug)]
pub(crate) struct Plane {
    pub(crate) handle: plane::Handle,
    pub(crate) facts: PlaneFacts,
    pub(crate) formats: FormatSet,
    pub(crate) props: PlaneProps,
}

#[derive(Debug)]
pub(crate) struct Resources {
    pub(crate) caps: Caps,
    pub(crate) crtcs: Vec<crtc::Handle>,
    pub(crate) crtc_props: Vec<CrtcProps>,
    pub(crate) planes: Vec<Plane>,
}

impl Resources {
    pub(crate) fn read(fd: &DrmFd) -> Result<Self> {
        fd.set_client_capability(ClientCapability::UniversalPlanes, true)
            .map_err(Error::AtomicUnsupported)?;
        fd.set_client_capability(ClientCapability::Atomic, true)
            .map_err(Error::AtomicUnsupported)?;
        let capability = |cap| fd.get_driver_capability(cap).unwrap_or(0);
        if capability(DriverCapability::CRTCInVBlankEvent) == 0 {
            return Err(Error::MissingCapability("CRTC ids in page-flip events"));
        }
        let cursor_side = |cap| match capability(cap) {
            0 => DEFAULT_CURSOR,
            side => side,
        };
        let caps = Caps {
            explicit_modifiers: capability(DriverCapability::AddFB2Modifiers) != 0,
            monotonic_timestamps: capability(DriverCapability::MonotonicTimestamp) != 0,
            async_flips: capability(DriverCapability::AtomicASyncPageFlip) != 0,
            cursor: (
                u32::try_from(cursor_side(DriverCapability::CursorWidth)).unwrap_or(64),
                u32::try_from(cursor_side(DriverCapability::CursorHeight)).unwrap_or(64),
            ),
        };

        let handles = fd.resource_handles().map_err(Error::Query)?;
        let crtcs = handles.crtcs().to_vec();
        let crtc_props = crtcs
            .iter()
            .map(|crtc| {
                CrtcProps::resolve(&ObjectProperties::read(fd, *crtc).map_err(Error::Query)?)
            })
            .collect::<Result<_>>()?;

        let mut planes = Vec::new();
        for handle in fd.plane_handles().map_err(Error::Query)? {
            let info = fd.get_plane(handle).map_err(Error::Query)?;
            let properties = ObjectProperties::read(fd, handle).map_err(Error::Query)?;
            let kind = match properties.raw("type") {
                Some(1) => PlaneKind::Primary,
                Some(2) => PlaneKind::Cursor,
                _ => PlaneKind::Overlay,
            };
            let possible_crtcs = handles
                .filter_crtcs(info.possible_crtcs())
                .iter()
                .filter_map(|crtc| crtcs.iter().position(|known| known == crtc))
                .fold(0u32, |mask, index| mask | (1 << index));
            let attached_to = info
                .crtc()
                .and_then(|crtc| crtcs.iter().position(|known| *known == crtc));
            let props = PlaneProps::resolve(&properties)?;
            planes.push(Plane {
                handle,
                facts: PlaneFacts {
                    kind,
                    possible_crtcs,
                    attached_to,
                    stacking: Stacking {
                        zpos: properties.raw("zpos"),
                        id: props.id,
                    },
                },
                formats: plane_formats(fd, &properties, info.formats()),
                props,
            });
        }

        Ok(Self {
            caps,
            crtcs,
            crtc_props,
            planes,
        })
    }

    pub(crate) fn crtc_index(&self, crtc: crtc::Handle) -> Option<usize> {
        self.crtcs.iter().position(|known| *known == crtc)
    }

    pub(crate) fn crtc_mask(&self, crtcs: &[crtc::Handle]) -> u32 {
        crtcs
            .iter()
            .filter_map(|crtc| self.crtc_index(*crtc))
            .fold(0, |mask, index| mask | (1 << index))
    }

    pub(crate) fn plane_facts(&self) -> Vec<PlaneFacts> {
        self.planes.iter().map(|plane| plane.facts).collect()
    }
}

fn plane_formats(fd: &DrmFd, props: &ObjectProperties, legacy: &[u32]) -> FormatSet {
    let blob = props
        .raw("IN_FORMATS")
        .filter(|blob| *blob != 0)
        .and_then(|blob| fd.get_property_blob(blob).ok())
        .and_then(|data| parse_in_formats(&data));
    match blob {
        Some(formats) => formats,
        None => legacy
            .iter()
            .map(|fourcc| Format::new(Fourcc(*fourcc), Modifier::INVALID))
            .collect(),
    }
}
