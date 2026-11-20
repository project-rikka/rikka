const BUCKET: u32 = 128;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sizing {
    pub visible: (u32, u32),
    pub scale: f64,
    pub interactive: bool,
}

impl Default for Sizing {
    fn default() -> Self {
        Self {
            visible: (1, 1),
            scale: 1.0,
            interactive: false,
        }
    }
}

impl Sizing {
    pub fn new(visible: (u32, u32), scale: f64, interactive: bool) -> Self {
        Self {
            visible: (visible.0.max(1), visible.1.max(1)),
            scale: if scale > 0.0 { scale } else { 1.0 },
            interactive,
        }
    }

    pub fn allocation(&self) -> (u32, u32) {
        match self.interactive {
            true => (bucket(self.visible.0), bucket(self.visible.1)),
            false => self.visible,
        }
    }

    pub fn fits(&self, size: (u32, u32)) -> bool {
        match self.interactive {
            true => roomy(size.0, self.visible.0) && roomy(size.1, self.visible.1),
            false => size == self.visible,
        }
    }

    pub fn destination(&self) -> (i32, i32) {
        let logical = |px: u32| (f64::from(px) / self.scale).round().max(1.0) as i32;
        (logical(self.visible.0), logical(self.visible.1))
    }
}

fn bucket(px: u32) -> u32 {
    px.max(1).div_ceil(BUCKET) * BUCKET
}

fn roomy(current: u32, requested: u32) -> bool {
    current >= requested && current - requested < 2 * BUCKET
}
