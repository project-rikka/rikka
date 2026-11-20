use rikka_compositor_geometry::{Logical, Size};

use crate::ids::LayerId;
use crate::shell::Shell;

impl Shell {
    pub fn configure_layer(&mut self, layer: LayerId, size: Size<i32, Logical>) -> Option<u32> {
        if !self.layers.get(layer).is_some_and(|record| !record.closed) {
            return None;
        }
        let serial = self.clock.serial();
        let record = self.layers.get_mut(layer)?;
        record
            .resource
            .configure(serial, size.w.unsigned_abs(), size.h.unsigned_abs());
        record.lifecycle.sent(serial, size);
        Some(serial)
    }

    pub fn close_layer(&mut self, layer: LayerId) {
        if let Some(record) = self.layers.get_mut(layer)
            && !record.closed
        {
            record.closed = true;
            record.resource.closed();
        }
    }
}
