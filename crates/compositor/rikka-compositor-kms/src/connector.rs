use drm::control::{Device as _, ModeFlags, ResourceHandles, connector, crtc};

use crate::drm_fd::DrmFd;
use crate::edid::{Edid, Identity};
use crate::ids::ConnectorId;
use crate::mode::Mode;
use crate::properties::{ConnectorProps, ObjectProperties};

#[derive(Debug, Clone)]
pub struct Connector {
    id: ConnectorId,
    name: String,
    modes: Vec<Mode>,
    size_mm: Option<(u32, u32)>,
    edid: Option<Edid>,
    vrr_capable: bool,
    max_bpc: Option<(u64, u64)>,
    non_desktop: bool,
    pub(crate) current_crtc: Option<crtc::Handle>,
    pub(crate) reachable_crtcs: Vec<crtc::Handle>,
    pub(crate) props: ConnectorProps,
}

impl Connector {
    pub(crate) fn read(
        fd: &DrmFd,
        handle: connector::Handle,
        resources: &ResourceHandles,
    ) -> Option<Self> {
        let info = fd.get_connector(handle, false).ok()?;
        if info.state() != connector::State::Connected || info.modes().is_empty() {
            return None;
        }
        let properties = ObjectProperties::read(fd, handle).ok()?;
        let props = ConnectorProps::resolve(&properties).ok()?;
        let name = name(&info);
        let edid = properties
            .raw("EDID")
            .filter(|blob| *blob != 0)
            .and_then(|blob| fd.get_property_blob(blob).ok())
            .and_then(|data| Edid::parse(&data));
        let encoders: Vec<_> = info
            .encoders()
            .iter()
            .filter_map(|encoder| fd.get_encoder(*encoder).ok())
            .collect();
        let current_crtc = info
            .current_encoder()
            .and_then(|encoder| fd.get_encoder(encoder).ok())
            .and_then(|encoder| encoder.crtc());
        let mut reachable_crtcs = Vec::new();
        for encoder in &encoders {
            for crtc in resources.filter_crtcs(encoder.possible_crtcs()) {
                if !reachable_crtcs.contains(&crtc) {
                    reachable_crtcs.push(crtc);
                }
            }
        }
        Some(Self {
            id: ConnectorId::from_handle(handle),
            name,
            modes: info.modes().iter().copied().map(Mode::new).collect(),
            size_mm: info.size(),
            edid,
            vrr_capable: properties.raw("vrr_capable") == Some(1),
            max_bpc: properties.range("max bpc"),
            non_desktop: properties.raw("non-desktop") == Some(1),
            current_crtc,
            reachable_crtcs,
            props,
        })
    }

    pub(crate) fn name_of(fd: &DrmFd, handle: connector::Handle) -> Option<String> {
        fd.get_connector(handle, false).ok().map(|info| name(&info))
    }

    pub fn id(&self) -> ConnectorId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn modes(&self) -> &[Mode] {
        &self.modes
    }

    pub fn mode_matching(&self, width: u32, height: u32, refresh_millihertz: u32) -> Option<Mode> {
        self.modes
            .iter()
            .filter(|mode| {
                mode.width() == width
                    && mode.height() == height
                    && mode.refresh_millihertz() == refresh_millihertz
            })
            .min_by_key(|mode| mode.raw().flags().contains(ModeFlags::INTERLACE))
            .copied()
    }

    pub fn size_mm(&self) -> Option<(u32, u32)> {
        self.size_mm
    }

    pub fn edid(&self) -> Option<&Edid> {
        self.edid.as_ref()
    }

    pub fn identity(&self) -> Identity {
        Identity::new(self.edid.as_ref(), &self.name)
    }

    pub fn vrr_capable(&self) -> bool {
        self.vrr_capable
    }

    pub fn max_bpc_range(&self) -> Option<(u64, u64)> {
        self.max_bpc
    }

    pub fn non_desktop(&self) -> bool {
        self.non_desktop
    }

    pub fn signals_pq(&self) -> bool {
        self.props.signals_pq()
    }
}

fn name(info: &connector::Info) -> String {
    format!("{}-{}", info.interface().as_str(), info.interface_id())
}
