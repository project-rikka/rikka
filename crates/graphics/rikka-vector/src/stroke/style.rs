#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Join {
    #[default]
    Miter,
    Round,
    Bevel,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Cap {
    #[default]
    Butt,
    Round,
    Square,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stroke<'a> {
    pub width: f32,
    pub join: Join,
    pub cap: Cap,
    pub miter_limit: f32,
    pub dashes: &'a [f32],
    pub dash_offset: f32,
}

impl Stroke<'_> {
    pub const DEFAULT_MITER_LIMIT: f32 = 4.0;

    #[must_use]
    pub const fn new(width: f32) -> Stroke<'static> {
        Stroke {
            width,
            join: Join::Miter,
            cap: Cap::Butt,
            miter_limit: Stroke::DEFAULT_MITER_LIMIT,
            dashes: &[],
            dash_offset: 0.0,
        }
    }

    #[must_use]
    pub const fn join(mut self, join: Join) -> Self {
        self.join = join;
        self
    }

    #[must_use]
    pub const fn cap(mut self, cap: Cap) -> Self {
        self.cap = cap;
        self
    }

    #[must_use]
    pub const fn miter_limit(mut self, limit: f32) -> Self {
        self.miter_limit = limit;
        self
    }

    #[must_use]
    pub fn dashed(self, dashes: &[f32], offset: f32) -> Stroke<'_> {
        Stroke {
            width: self.width,
            join: self.join,
            cap: self.cap,
            miter_limit: self.miter_limit,
            dashes,
            dash_offset: offset,
        }
    }

    pub(crate) fn is_dashed(&self) -> bool {
        let total: f32 = self.dashes.iter().sum();
        !self.dashes.is_empty()
            && total > 0.0
            && total.is_finite()
            && self.dashes.iter().all(|dash| *dash >= 0.0)
    }
}
