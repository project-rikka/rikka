use std::sync::Arc;

use animate_fluid::{Emitter, EmitterId, FluidSettings};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Liquids {
    pub emitters: Vec<Arc<Emitter>>,
    pub settings: FluidSettings,
}

impl Liquids {
    pub fn emitter(&self, id: EmitterId) -> Option<&Arc<Emitter>> {
        self.emitters.iter().find(|emitter| emitter.id == id)
    }
}
