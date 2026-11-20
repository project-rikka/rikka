pub const HAIR_LAYER: u32 = 1 << 28;
pub const FACE_LAYER: u32 = 1 << 29;
pub const SEE_THROUGH_LAYER: u32 = 1 << 30;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Characters {
    pub hair_layers: u32,
    pub face_layers: u32,
    pub see_through_layers: u32,
    pub facial_layers: u32,
}

impl Default for Characters {
    fn default() -> Self {
        Self {
            hair_layers: HAIR_LAYER,
            face_layers: FACE_LAYER,
            see_through_layers: SEE_THROUGH_LAYER,
            facial_layers: SEE_THROUGH_LAYER,
        }
    }
}

impl Characters {
    pub(crate) fn hair(&self, layers: u32) -> bool {
        layers & self.hair_layers != 0
    }

    pub(crate) fn face(&self, layers: u32) -> bool {
        layers & (self.face_layers | self.see_through_layers) != 0
    }

    pub(crate) fn see_through(&self, layers: u32) -> bool {
        layers & self.see_through_layers != 0
    }

    pub(crate) fn facial(&self, layers: u32) -> bool {
        layers & self.facial_layers != 0
    }
}
