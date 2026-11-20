use crate::blob::Blob;
use crate::cursor::CursorImage;
use crate::ids::CrtcId;
use crate::mode::Mode;
use crate::present::Present;
use crate::properties::{ConnectorProps, CrtcProps, PlaneProps, Signalling};
use crate::resources::Caps;
use crate::timing::VblankTimeline;

#[derive(Debug)]
pub(crate) enum Command {
    Enable(Box<Setup>),
    Disable(CrtcId),
    Vrr {
        crtc: CrtcId,
        enabled: bool,
    },
    Present(Present),
    Cursor {
        crtc: CrtcId,
        image: Option<CursorImage>,
    },
    Pause,
    Resume(Vec<ConnectorProps>),
    Stop,
}

#[derive(Debug)]
pub(crate) struct Setup {
    pub(crate) crtc: CrtcId,
    pub(crate) crtc_props: CrtcProps,
    pub(crate) connector: ConnectorProps,
    pub(crate) mode: Mode,
    pub(crate) blob: Blob,
    pub(crate) metadata: Option<Blob>,
    pub(crate) primary: PlaneProps,
    pub(crate) cursor: Option<PlaneProps>,
    pub(crate) overlay: Option<PlaneProps>,
    pub(crate) vrr: bool,
    pub(crate) max_bpc: Option<u64>,
    pub(crate) timeline: VblankTimeline,
}

#[derive(Debug, Clone)]
pub(crate) struct Hardware {
    pub(crate) caps: Caps,
    pub(crate) crtcs: Vec<CrtcProps>,
    pub(crate) planes: Vec<PlaneProps>,
    pub(crate) connectors: Vec<ConnectorProps>,
}

impl Setup {
    pub(crate) fn signalling(&self) -> Signalling {
        match &self.metadata {
            Some(blob) => Signalling::Pq {
                metadata: blob.id(),
            },
            None => Signalling::Sdr,
        }
    }
}
