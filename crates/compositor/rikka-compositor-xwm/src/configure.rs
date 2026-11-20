use crate::event::ConfigureSerial;
use crate::geometry::Geometry;
use crate::window::XWindowId;

#[derive(Debug, Clone, Copy)]
struct Pending {
    window: XWindowId,
    sequence: u64,
    geometry: Geometry,
    serial: ConfigureSerial,
}

#[derive(Debug, Default)]
pub(crate) struct Configures {
    pending: Vec<Pending>,
    issued: u64,
}

impl Configures {
    pub(crate) fn next_serial(&mut self) -> ConfigureSerial {
        self.issued += 1;
        ConfigureSerial(self.issued)
    }

    pub(crate) fn target(&self, window: XWindowId, settled: Geometry) -> Geometry {
        self.pending
            .iter()
            .rev()
            .find(|pending| pending.window == window)
            .map_or(settled, |pending| pending.geometry)
    }

    pub(crate) fn expect(
        &mut self,
        window: XWindowId,
        sequence: u64,
        geometry: Geometry,
        serial: ConfigureSerial,
    ) {
        self.pending.push(Pending {
            window,
            sequence,
            geometry,
            serial,
        });
    }

    pub(crate) fn settle(
        &mut self,
        window: XWindowId,
        sequence: u64,
        geometry: Geometry,
    ) -> Option<ConfigureSerial> {
        let accepted = self
            .pending
            .iter()
            .rev()
            .find(|pending| {
                pending.window == window
                    && pending.sequence == sequence
                    && pending.geometry == geometry
            })?
            .serial;
        self.pending
            .retain(|pending| pending.window != window || pending.sequence > sequence);
        Some(accepted)
    }

    pub(crate) fn forget(&mut self, window: XWindowId) {
        self.pending.retain(|pending| pending.window != window);
    }
}
