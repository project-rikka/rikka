mod announce;
mod info;
mod pad;
mod protocol;
mod tool;

use rikka_compositor_protocols::zwp_tablet_pad_dial_v2::ZwpTabletPadDialV2;
use rikka_compositor_protocols::zwp_tablet_pad_group_v2::ZwpTabletPadGroupV2;
use rikka_compositor_protocols::zwp_tablet_pad_ring_v2::ZwpTabletPadRingV2;
use rikka_compositor_protocols::zwp_tablet_pad_strip_v2::ZwpTabletPadStripV2;
use rikka_compositor_protocols::zwp_tablet_pad_v2::ZwpTabletPadV2;
use rikka_compositor_protocols::zwp_tablet_seat_v2::ZwpTabletSeatV2;
use rikka_compositor_protocols::zwp_tablet_tool_v2::ZwpTabletToolV2;
use rikka_compositor_protocols::zwp_tablet_v2::ZwpTabletV2;
use rikka_compositor_server::ByClient;
use rikka_compositor_surface::SurfaceId;
use rikka_compositor_types::ToolInfo;
use rikka_wayland_wire::ClientId;
use slotmap::SlotMap;

use crate::focus::Focused;
use crate::ids::{PadId, TabletId, ToolId};
use crate::pointer::CursorImage;

pub(crate) use announce::{add_pad, add_tablet, add_tool, remove_pad, remove_tablet, remove_tool};
pub use info::{Bus, PadGroupInfo, PadInfo, TabletInfo, ToolMotion};
pub(crate) use pad::{pad_focus, pad_input, pads_surface_gone};
pub(crate) use protocol::{
    pad_part_destroyed, pad_part_request, tablet_destroyed, tablet_manager, tablet_request,
    tablet_seat_destroyed, tablet_seat_request, tool_destroyed, tool_request,
};
pub(crate) use tool::{tool_button, tool_motion, tool_out, tool_tip, tools_surface_gone};

#[derive(Debug)]
pub(crate) struct Tablet {
    pub(crate) info: TabletInfo,
    pub(crate) resources: Vec<ZwpTabletV2>,
}

#[derive(Debug)]
pub(crate) struct Tool {
    pub(crate) info: ToolInfo,
    pub(crate) resources: ByClient<ZwpTabletToolV2>,
    pub(crate) tablet: Option<TabletId>,
    pub(crate) focused: Option<Focused>,
    pub(crate) proximity_serial: u32,
    pub(crate) down: bool,
    pub(crate) cursor: CursorImage,
}

#[derive(Debug)]
pub(crate) struct PadObjects {
    pub(crate) pad: ZwpTabletPadV2,
    pub(crate) groups: Vec<GroupObjects>,
}

#[derive(Debug)]
pub(crate) struct GroupObjects {
    pub(crate) group: ZwpTabletPadGroupV2,
    pub(crate) rings: Vec<(u32, ZwpTabletPadRingV2)>,
    pub(crate) strips: Vec<(u32, ZwpTabletPadStripV2)>,
    pub(crate) dials: Vec<(u32, ZwpTabletPadDialV2)>,
}

#[derive(Debug)]
pub(crate) struct Pad {
    pub(crate) info: PadInfo,
    pub(crate) tablet: TabletId,
    pub(crate) objects: Vec<PadObjects>,
    pub(crate) focus: Option<SurfaceId>,
    pub(crate) modes: Vec<u32>,
}

#[derive(Debug, Default)]
pub struct Tablets {
    pub(crate) seats: Vec<ZwpTabletSeatV2>,
    pub(crate) tablets: SlotMap<TabletId, Tablet>,
    pub(crate) tools: SlotMap<ToolId, Tool>,
    pub(crate) pads: SlotMap<PadId, Pad>,
}

impl Tablets {
    pub fn tablets(&self) -> impl Iterator<Item = (TabletId, &TabletInfo)> {
        self.tablets.iter().map(|(id, tablet)| (id, &tablet.info))
    }

    pub fn tool_cursor(&self, tool: ToolId) -> Option<CursorImage> {
        self.tools.get(tool).map(|tool| tool.cursor)
    }

    pub(crate) fn tablet_of(&self, tablet: TabletId, client: ClientId) -> Option<&ZwpTabletV2> {
        self.tablets
            .get(tablet)?
            .resources
            .iter()
            .find(|resource| resource.client() == client && resource.alive())
    }
}
