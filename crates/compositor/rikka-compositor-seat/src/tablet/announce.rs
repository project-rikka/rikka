use rikka_compositor_protocols::zwp_tablet_pad_dial_v2::ZwpTabletPadDialV2;
use rikka_compositor_protocols::zwp_tablet_pad_group_v2::ZwpTabletPadGroupV2;
use rikka_compositor_protocols::zwp_tablet_pad_ring_v2::ZwpTabletPadRingV2;
use rikka_compositor_protocols::zwp_tablet_pad_strip_v2::ZwpTabletPadStripV2;
use rikka_compositor_protocols::zwp_tablet_pad_v2::ZwpTabletPadV2;
use rikka_compositor_protocols::zwp_tablet_seat_v2::ZwpTabletSeatV2;
use rikka_compositor_protocols::zwp_tablet_tool_v2::{Capability, Type, ZwpTabletToolV2};
use rikka_compositor_protocols::zwp_tablet_v2::{Bustype, ZwpTabletV2};
use rikka_compositor_server::ByClient;
use rikka_compositor_server::Client;
use rikka_compositor_types::{ToolCapabilities, ToolInfo, ToolKind};
use rikka_wayland_wire::Resource;

use crate::access::Access;
use crate::focus::Focused;
use crate::ids::{PadId, SeatId, TabletId, ToolId};
use crate::pointer::CursorImage;
use crate::tablet::info::{Bus, PadInfo, TabletInfo};
use crate::tablet::{GroupObjects, Pad, PadObjects, Tablet, Tablets, Tool};

pub(crate) fn add_tablet<A: Access>(
    state: &mut A,
    seat: SeatId,
    info: TabletInfo,
) -> Option<TabletId> {
    let entry = state.seats().seat_mut(seat)?;
    let tablets = &mut entry.tablets;
    let id = tablets.tablets.insert(Tablet {
        info,
        resources: Vec::new(),
    });
    for tablet_seat in tablets.seats.clone() {
        announce_tablet(tablets, seat, id, &tablet_seat);
    }
    Some(id)
}

pub(crate) fn remove_tablet<A: Access>(state: &mut A, seat: SeatId, tablet: TabletId) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let Some(removed) = entry.tablets.tablets.remove(tablet) else {
        return;
    };
    for resource in removed.resources {
        resource.removed();
    }
}

pub(crate) fn add_tool<A: Access>(state: &mut A, seat: SeatId, info: ToolInfo) -> Option<ToolId> {
    let entry = state.seats().seat_mut(seat)?;
    let tablets = &mut entry.tablets;
    let id = tablets.tools.insert(Tool {
        info,
        resources: ByClient::default(),
        tablet: None,
        focused: None,
        proximity_serial: 0,
        down: false,
        cursor: CursorImage::default(),
    });
    for tablet_seat in tablets.seats.clone() {
        announce_tool(tablets, seat, id, &tablet_seat);
    }
    Some(id)
}

pub(crate) fn remove_tool<A: Access>(state: &mut A, seat: SeatId, tool: ToolId) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let Some(removed) = entry.tablets.tools.remove(tool) else {
        return;
    };
    let focused = removed.focused.as_ref().map(Focused::client);
    for resource in removed.resources.all() {
        if Some(resource.client()) == focused {
            resource.proximity_out();
        }
        resource.removed();
    }
}

pub(crate) fn add_pad<A: Access>(
    state: &mut A,
    seat: SeatId,
    tablet: TabletId,
    info: PadInfo,
) -> Option<PadId> {
    let entry = state.seats().seat_mut(seat)?;
    let tablets = &mut entry.tablets;
    let modes = vec![0; info.groups.len()];
    let id = tablets.pads.insert(Pad {
        info,
        tablet,
        objects: Vec::new(),
        focus: None,
        modes,
    });
    for tablet_seat in tablets.seats.clone() {
        announce_pad(tablets, seat, id, &tablet_seat);
    }
    Some(id)
}

pub(crate) fn remove_pad<A: Access>(state: &mut A, seat: SeatId, pad: PadId) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let Some(removed) = entry.tablets.pads.remove(pad) else {
        return;
    };
    for objects in removed.objects {
        objects.pad.removed();
    }
}

pub(crate) fn announce_all(tablets: &mut Tablets, seat: SeatId, tablet_seat: &ZwpTabletSeatV2) {
    let ids: Vec<TabletId> = tablets.tablets.keys().collect();
    for id in ids {
        announce_tablet(tablets, seat, id, tablet_seat);
    }
    let ids: Vec<ToolId> = tablets.tools.keys().collect();
    for id in ids {
        announce_tool(tablets, seat, id, tablet_seat);
    }
    let ids: Vec<PadId> = tablets.pads.keys().collect();
    for id in ids {
        announce_pad(tablets, seat, id, tablet_seat);
    }
}

fn create<R: Resource>(parent: &impl Resource) -> Option<R> {
    let client = Client::from_resource(parent)?;
    Some(client.create::<R>(parent.object().version()))
}

fn announce_tablet(
    tablets: &mut Tablets,
    seat: SeatId,
    id: TabletId,
    tablet_seat: &ZwpTabletSeatV2,
) {
    let Some(tablet) = tablets.tablets.get_mut(id) else {
        return;
    };
    let Some(resource) = create::<ZwpTabletV2>(tablet_seat) else {
        return;
    };
    resource.set_key(seat);
    resource.set_key(id);
    tablet_seat.tablet_added(&resource);
    resource.name(&tablet.info.name);
    resource.send_id(tablet.info.vendor, tablet.info.product);
    if let Some(path) = &tablet.info.path {
        resource.path(path);
    }
    if let Some(bus) = tablet.info.bus {
        resource.bustype(bustype(bus));
    }
    resource.done();
    tablet.resources.push(resource);
}

fn announce_tool(tablets: &mut Tablets, seat: SeatId, id: ToolId, tablet_seat: &ZwpTabletSeatV2) {
    let Some(tool) = tablets.tools.get_mut(id) else {
        return;
    };
    let Some(resource) = create::<ZwpTabletToolV2>(tablet_seat) else {
        return;
    };
    resource.set_key(seat);
    resource.set_key(id);
    tablet_seat.tool_added(&resource);
    let info = tool.info;
    resource.r#type(kind(info.kind));
    if info.serial != 0 {
        resource.hardware_serial((info.serial >> 32) as u32, info.serial as u32);
    }
    if info.hardware_id != 0 {
        resource.hardware_id_wacom((info.hardware_id >> 32) as u32, info.hardware_id as u32);
    }
    for (flag, capability) in [
        (ToolCapabilities::TILT, Capability::Tilt),
        (ToolCapabilities::PRESSURE, Capability::Pressure),
        (ToolCapabilities::DISTANCE, Capability::Distance),
        (ToolCapabilities::ROTATION, Capability::Rotation),
        (ToolCapabilities::SLIDER, Capability::Slider),
        (ToolCapabilities::WHEEL, Capability::Wheel),
    ] {
        if info.capabilities.contains(flag) {
            resource.capability(capability);
        }
    }
    resource.done();
    tool.resources.add(resource);
}

fn announce_pad(tablets: &mut Tablets, seat: SeatId, id: PadId, tablet_seat: &ZwpTabletSeatV2) {
    let Some(pad) = tablets.pads.get_mut(id) else {
        return;
    };
    let Some(resource) = create::<ZwpTabletPadV2>(tablet_seat) else {
        return;
    };
    resource.set_key(seat);
    resource.set_key(id);
    tablet_seat.pad_added(&resource);
    if let Some(path) = &pad.info.path {
        resource.path(path);
    }
    resource.buttons(pad.info.buttons);
    let mut groups = Vec::new();
    for info in &pad.info.groups {
        let Some(group) = create::<ZwpTabletPadGroupV2>(&resource) else {
            continue;
        };
        group.set_key(seat);
        group.set_key(id);
        resource.group(&group);
        let buttons: Vec<u8> = info
            .buttons
            .iter()
            .flat_map(|button| button.to_ne_bytes())
            .collect();
        group.buttons(&buttons);
        let mut objects = GroupObjects {
            group: group.clone(),
            rings: Vec::new(),
            strips: Vec::new(),
            dials: Vec::new(),
        };
        for ring in &info.rings {
            if let Some(created) = create::<ZwpTabletPadRingV2>(&group) {
                created.set_key(seat);
                created.set_key(id);
                group.ring(&created);
                objects.rings.push((*ring, created));
            }
        }
        for strip in &info.strips {
            if let Some(created) = create::<ZwpTabletPadStripV2>(&group) {
                created.set_key(seat);
                created.set_key(id);
                group.strip(&created);
                objects.strips.push((*strip, created));
            }
        }
        if group.version() >= 2 {
            for dial in &info.dials {
                if let Some(created) = create::<ZwpTabletPadDialV2>(&group) {
                    created.set_key(seat);
                    created.set_key(id);
                    group.dial(&created);
                    objects.dials.push((*dial, created));
                }
            }
        }
        group.modes(info.modes);
        group.done();
        groups.push(objects);
    }
    resource.done();
    pad.objects.push(PadObjects {
        pad: resource,
        groups,
    });
}

fn bustype(bus: Bus) -> Bustype {
    match bus {
        Bus::Usb => Bustype::Usb,
        Bus::Bluetooth => Bustype::Bluetooth,
        Bus::Virtual => Bustype::Virtual,
        Bus::Serial => Bustype::Serial,
        Bus::I2c => Bustype::I2c,
    }
}

fn kind(kind: ToolKind) -> Type {
    match kind {
        ToolKind::Pen => Type::Pen,
        ToolKind::Eraser => Type::Eraser,
        ToolKind::Brush => Type::Brush,
        ToolKind::Pencil => Type::Pencil,
        ToolKind::Airbrush => Type::Airbrush,
        ToolKind::Finger => Type::Finger,
        ToolKind::Mouse => Type::Mouse,
        ToolKind::Lens => Type::Lens,
    }
}
