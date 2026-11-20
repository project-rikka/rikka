const BOLD_ADVANCE_EM: f32 = 1.0 / 24.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Synthesis {
    pub bold: bool,
    pub oblique: bool,
}

impl Synthesis {
    pub const NONE: Self = Self {
        bold: false,
        oblique: false,
    };

    #[inline]
    pub fn extra_advance_em(self) -> f32 {
        if self.bold { BOLD_ADVANCE_EM } else { 0.0 }
    }
}
