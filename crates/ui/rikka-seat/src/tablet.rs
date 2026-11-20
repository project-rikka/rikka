use rikka_types::{CursorShape, Modifiers};
use rikka_wayland_client::wl_surface::WlSurface;
use rikka_wayland_client::zwp_tablet_seat_v2::{self, ZwpTabletSeatV2};
use rikka_wayland_client::zwp_tablet_v2::{self, Bustype, ZwpTabletV2};

use crate::cursor::{Cursors, Owner};
use crate::pad::PadDevice;
use crate::seat_event::SeatEvent;
use crate::tool::ToolDevice;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Bus {
    Usb,
    Bluetooth,
    Virtual,
    Serial,
    I2c,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tablet {
    pub name: String,
    pub vendor: u32,
    pub product: u32,
    pub paths: Vec<String>,
    pub bus: Option<Bus>,
}

#[derive(Debug)]
pub(crate) struct TabletSeat {
    seat: ZwpTabletSeatV2,
    pub(crate) tablets: Vec<(ZwpTabletV2, Tablet)>,
    pub(crate) tools: Vec<ToolDevice>,
    pub(crate) pads: Vec<PadDevice>,
}

impl TabletSeat {
    pub(crate) fn new(seat: ZwpTabletSeatV2) -> TabletSeat {
        TabletSeat {
            seat,
            tablets: Vec::new(),
            tools: Vec::new(),
            pads: Vec::new(),
        }
    }

    pub(crate) fn is(&self, seat: &ZwpTabletSeatV2) -> bool {
        self.seat == *seat
    }

    pub(crate) fn release(self) {
        for (tablet, _) in self.tablets {
            tablet.destroy();
        }
        for tool in self.tools {
            tool.release();
        }
        for pad in self.pads {
            pad.release();
        }
        self.seat.destroy();
    }

    pub(crate) fn added(&mut self, event: zwp_tablet_seat_v2::Event, cursors: &Cursors) {
        match event {
            zwp_tablet_seat_v2::Event::TabletAdded { id } => {
                self.tablets.push((id, Tablet::default()));
            }
            zwp_tablet_seat_v2::Event::ToolAdded { id } => {
                let cursor = cursors.device(Owner::Tool(&id));
                self.tools.push(ToolDevice::new(id, cursor));
            }
            zwp_tablet_seat_v2::Event::PadAdded { id } => self.pads.push(PadDevice::new(id)),
            _ => {}
        }
    }

    pub(crate) fn describe(&mut self, proxy: &ZwpTabletV2, event: zwp_tablet_v2::Event) -> bool {
        let Some(at) = self.tablets.iter().position(|(tablet, _)| tablet == proxy) else {
            return false;
        };
        let info = &mut self.tablets[at].1;
        match event {
            zwp_tablet_v2::Event::Name { name } => info.name = name,
            zwp_tablet_v2::Event::Id { vid, pid } => {
                info.vendor = vid;
                info.product = pid;
            }
            zwp_tablet_v2::Event::Path { path } => info.paths.push(path),
            zwp_tablet_v2::Event::Bustype { bustype } => {
                info.bus = match bustype {
                    Ok(Bustype::Usb) => Some(Bus::Usb),
                    Ok(Bustype::Bluetooth) => Some(Bus::Bluetooth),
                    Ok(Bustype::Virtual) => Some(Bus::Virtual),
                    Ok(Bustype::Serial) => Some(Bus::Serial),
                    Ok(Bustype::I2c) => Some(Bus::I2c),
                    Err(unknown) => {
                        tracing::debug!("unknown tablet bus type: {unknown:?}");
                        None
                    }
                };
            }
            zwp_tablet_v2::Event::Removed => {
                let (tablet, _) = self.tablets.remove(at);
                tablet.destroy();
            }
            _ => {}
        }
        true
    }

    pub(crate) fn flush(&mut self, out: &mut Vec<SeatEvent>) {
        for tool in &mut self.tools {
            tool.flush(out);
        }
    }

    pub(crate) fn remove_tool(
        &mut self,
        at: usize,
        modifiers: Modifiers,
        out: &mut Vec<SeatEvent>,
    ) {
        let mut tool = self.tools.remove(at);
        tool.removed(modifiers, out);
        tool.release();
    }

    pub(crate) fn remove_pad(&mut self, at: usize, out: &mut Vec<SeatEvent>) {
        let mut pad = self.pads.remove(at);
        pad.removed(out);
        pad.release();
    }

    pub(crate) fn show(
        &mut self,
        surface: &WlSurface,
        shape: CursorShape,
        scale: u32,
        cursors: &mut Cursors,
    ) {
        for tool in &mut self.tools {
            tool.show(surface, shape, scale, cursors);
        }
    }
}
