#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Levels {
    pub(crate) depth: usize,
    pub(crate) layers: usize,
    pub(crate) nesting: usize,
}

impl Levels {
    pub(crate) const LIMIT: Levels = Levels {
        depth: 512,
        layers: 32,
        nesting: 8,
    };

    pub(crate) fn highest(self, other: Levels) -> Levels {
        Levels {
            depth: self.depth.max(other.depth),
            layers: self.layers.max(other.layers),
            nesting: self.nesting.max(other.nesting),
        }
    }

    pub(crate) fn above(self, base: Levels) -> Levels {
        Levels {
            depth: self.depth.saturating_sub(base.depth),
            layers: self.layers.saturating_sub(base.layers),
            nesting: self.nesting.saturating_sub(base.nesting),
        }
    }

    pub(crate) fn plus(self, extent: Levels) -> Levels {
        Levels {
            depth: self.depth + extent.depth,
            layers: self.layers + extent.layers,
            nesting: self.nesting + extent.nesting,
        }
    }

    pub(crate) fn fits(self) -> bool {
        self.depth <= Levels::LIMIT.depth
            && self.layers <= Levels::LIMIT.layers
            && self.nesting <= Levels::LIMIT.nesting
    }
}
