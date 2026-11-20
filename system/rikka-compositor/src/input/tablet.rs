use std::collections::HashMap;
use std::time::Duration;

use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_input::{Capabilities, DeviceId, DeviceInfo, ToolEvent, ToolState};
use rikka_compositor_seat::{
    Button, Motion, PadGroupInfo, PadId, PadInfo, TabletId, TabletInfo, ToolId, ToolMotion,
};
use rikka_compositor_surface::SurfaceId;
use rikka_compositor_types::{PadInput, Press, ToolInfo};
use rikka_input::evdev::BTN_LEFT;

use crate::context::{InputCtx, SwitcherCtx, switcher_ctx, windows_ctx};
use crate::context::{input_ref, person, windows_ref};
use crate::state::Compositor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TabletIdentity {
    tablet: TabletId,
    vendor: u32,
    product: u32,
}

#[derive(Debug, Default)]
pub(crate) struct TabletDevices {
    tablets: HashMap<DeviceId, TabletIdentity>,
    pads: HashMap<DeviceId, (PadId, Vec<u32>)>,
    tools: HashMap<(u64, u64), ToolId>,
    proximity_tool: Option<ToolId>,
    relaying: bool,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ToolRelay {
    location: Point<f64, Logical>,
    tip: Option<Press>,
    time: Duration,
}

impl TabletDevices {
    pub(crate) fn proximity_tool(&self) -> Option<ToolId> {
        self.proximity_tool
    }

    pub(crate) fn pads(&self) -> impl Iterator<Item = PadId> + '_ {
        self.pads.values().map(|(pad, _)| *pad)
    }
}

fn tablet_info(device: &DeviceInfo) -> TabletInfo {
    TabletInfo {
        name: device.name.clone(),
        vendor: device.vendor,
        product: device.product,
        path: device_path(device),
        bus: None,
    }
}

fn device_path(device: &DeviceInfo) -> Option<String> {
    (!device.sysname.is_empty()).then(|| format!("/dev/input/{}", device.sysname))
}

impl SwitcherCtx<'_> {
    pub(super) fn tablet_added(&mut self, device: &DeviceInfo) {
        if device.capabilities.contains(Capabilities::TABLET_TOOL) {
            let info = tablet_info(device);
            if let Some(tablet) = person!(self).add_tablet(info) {
                self.input.tablets.tablets.insert(
                    device.id,
                    TabletIdentity {
                        tablet,
                        vendor: device.vendor,
                        product: device.product,
                    },
                );
            }
        }
        if let Some(layout) = device
            .pad
            .as_ref()
            .filter(|_| device.capabilities.contains(Capabilities::TABLET_PAD))
        {
            let known = self.input.tablets.tablets.values();
            let sibling = known
                .clone()
                .find(|known| known.vendor == device.vendor && known.product == device.product)
                .or_else(|| known.clone().next())
                .map(|known| known.tablet);
            let tablet = match sibling {
                Some(tablet) => Some(tablet),
                None => person!(self).add_tablet(tablet_info(device)),
            };
            let Some(tablet) = tablet else {
                return;
            };
            let info = PadInfo {
                path: device_path(device),
                buttons: layout.buttons,
                groups: layout
                    .groups
                    .iter()
                    .map(|group| PadGroupInfo {
                        buttons: group.buttons.clone(),
                        rings: group.rings.clone(),
                        strips: group.strips.clone(),
                        dials: Vec::new(),
                        modes: group.modes,
                    })
                    .collect(),
            };
            let modes = layout.groups.iter().map(|group| group.mode).collect();
            let focus = windows_ref!(self)
                .keyboard_focus()
                .map(|focus| focus.surface());
            let mut pads = person!(self);
            if let Some(pad) = pads.add_pad(tablet, info) {
                pads.pad_focus(pad, focus);
                self.input.tablets.pads.insert(device.id, (pad, modes));
            }
        }
    }

    pub(super) fn tablet_removed(&mut self, device: DeviceId) {
        if let Some((pad, _)) = self.input.tablets.pads.remove(&device) {
            person!(self).remove_pad(pad);
        }
        if let Some(removed) = self.input.tablets.tablets.remove(&device) {
            person!(self).remove_tablet(removed.tablet);
        }
    }

    pub(super) fn set_pad_focus(&mut self, focus: Option<SurfaceId>) {
        let pads: Vec<PadId> = self.input.tablets.pads().collect();
        let mut seat = person!(self);
        for pad in pads {
            seat.pad_focus(pad, focus);
        }
    }

    pub(super) fn handle_pad_input(
        &mut self,
        device: DeviceId,
        input: PadInput,
        (group, mode): (u32, u32),
        time: Duration,
    ) {
        let Some((pad, modes)) = self.input.tablets.pads.get_mut(&device) else {
            return;
        };
        let pad = *pad;
        let switched = match modes.get_mut(group as usize) {
            Some(current) if *current != mode => {
                *current = mode;
                true
            }
            _ => false,
        };
        let mut pads = person!(self);
        pads.activity();
        if switched {
            pads.pad_input(pad, PadInput::Mode { group, mode }, time);
        }
        pads.pad_input(pad, input, time);
    }

    fn tool_id(&mut self, tool: ToolInfo) -> Option<ToolId> {
        let key = (tool.serial, tool.hardware_id);
        if let Some(id) = self.input.tablets.tools.get(&key) {
            return Some(*id);
        }
        let id = person!(self).add_tool(tool)?;
        self.input.tablets.tools.insert(key, id);
        Some(id)
    }
}

impl InputCtx<'_> {
    pub(super) fn handle_tool_event(
        &mut self,
        device: DeviceId,
        tool: ToolInfo,
        tool_event: ToolEvent,
        state: &ToolState,
        time: Duration,
    ) -> Option<ToolRelay> {
        let seat = self.input.seat;
        let tablet = self
            .input
            .tablets
            .tablets
            .get(&device)
            .map(|known| known.tablet)?;
        let id = switcher_ctx!(self).tool_id(tool)?;
        person!(self).activity();
        match tool_event {
            ToolEvent::ProximityOut => {
                person!(self).tool_out(id, time);
                if self.input.tablets.proximity_tool == Some(id) {
                    self.input.tablets.proximity_tool = None;
                    let image = self
                        .seats
                        .get(seat)
                        .map(|seat_state| seat_state.pointer().cursor())
                        .unwrap_or_default();
                    self.cursor_shown(image);
                }
            }
            ToolEvent::ProximityIn | ToolEvent::Motion | ToolEvent::Tip(_) => {
                let location = super::absolute::place(
                    self.outputs,
                    &self.input.pending,
                    device,
                    state.position,
                )?;
                let tip = match tool_event {
                    ToolEvent::Tip(press) => Some(press),
                    _ => None,
                };
                let on_caption =
                    tip == Some(Press::Pressed) && input_ref!(self).caption_at(location).is_some();
                if self.input.tablets.relaying || on_caption {
                    return Some(self.relaying_tool(location, tip, time));
                }
                if tool_event == ToolEvent::Tip(Press::Pressed) {
                    windows_ctx!(self).focus_under(location);
                }
                let under = input_ref!(self).focus_at(location);
                self.cursor_to(location);
                let mut pen = person!(self);
                pen.tool_motion(
                    id,
                    tablet,
                    ToolMotion {
                        location,
                        under,
                        axes: state.axes(),
                        time,
                    },
                );
                if let ToolEvent::Tip(press) = tool_event {
                    pen.tool_tip(id, press, time);
                }
                if tool_event == ToolEvent::ProximityIn {
                    self.input.tablets.proximity_tool = Some(id);
                    let image = self
                        .seats
                        .get(seat)
                        .and_then(|seat_state| seat_state.tablets().tool_cursor(id))
                        .unwrap_or_default();
                    self.cursor_shown(image);
                }
            }
            ToolEvent::Button { code, press } => {
                person!(self).tool_button(id, code, press, time);
            }
        }
        None
    }

    fn relaying_tool(
        &mut self,
        location: Point<f64, Logical>,
        tip: Option<Press>,
        time: Duration,
    ) -> ToolRelay {
        self.cursor_to(location);
        match tip {
            Some(Press::Pressed) => {
                self.input.tablets.relaying = true;
                windows_ctx!(self).focus_under(location);
                windows_ctx!(self).press_caption(BTN_LEFT, location, time);
            }
            Some(Press::Released) => self.input.tablets.relaying = false,
            None => {}
        }
        ToolRelay {
            location,
            tip,
            time,
        }
    }
}

impl Compositor {
    pub(super) fn relay_tool(&mut self, relay: ToolRelay) {
        let mut pointer = self.person();
        pointer.motion(Motion {
            location: relay.location,
            under: None,
            time: relay.time,
        });
        if let Some(press) = relay.tip {
            pointer.button(Button {
                code: BTN_LEFT,
                press,
                time: relay.time,
            });
        }
        pointer.frame();
    }
}
