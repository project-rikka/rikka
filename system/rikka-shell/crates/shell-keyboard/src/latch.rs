use rikka_types::Modifiers;

use crate::layout::Modifier;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Latch {
    #[default]
    Off,
    Once,
    Locked,
}

impl Latch {
    fn pressed(self) -> Latch {
        match self {
            Latch::Off => Latch::Once,
            Latch::Once => Latch::Locked,
            Latch::Locked => Latch::Off,
        }
    }

    fn is_on(self) -> bool {
        self != Latch::Off
    }

    fn spent(self) -> Latch {
        match self {
            Latch::Once => Latch::Off,
            active_modifiers => active_modifiers,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Latches {
    shift: Latch,
    control: Latch,
    alt: Latch,
    logo: Latch,
    caps: bool,
}

impl Latches {
    pub(crate) fn press(&mut self, modifier: Modifier) {
        let latch = self.latch_mut(modifier);
        *latch = latch.pressed();
    }

    pub(crate) fn lock(&mut self) {
        self.caps = !self.caps;
    }

    pub(super) fn spend(&mut self) -> bool {
        let before = *self;
        self.shift = self.shift.spent();
        self.control = self.control.spent();
        self.alt = self.alt.spent();
        self.logo = self.logo.spent();
        *self != before
    }

    #[must_use]
    pub(crate) fn active_modifiers(&self) -> Modifiers {
        Modifiers {
            shift: self.shift.is_on(),
            control: self.control.is_on(),
            alt: self.alt.is_on(),
            logo: self.logo.is_on(),
        }
    }

    #[must_use]
    pub(super) fn is_locked(&self) -> bool {
        self.caps
    }

    #[must_use]
    pub(super) fn latch(&self, modifier: Modifier) -> Latch {
        match modifier {
            Modifier::Shift => self.shift,
            Modifier::Control => self.control,
            Modifier::Alt => self.alt,
            Modifier::Logo => self.logo,
        }
    }

    #[must_use]
    pub(super) fn is_upper(&self) -> bool {
        self.shift.is_on() != self.caps
    }

    fn latch_mut(&mut self, modifier: Modifier) -> &mut Latch {
        match modifier {
            Modifier::Shift => &mut self.shift,
            Modifier::Control => &mut self.control,
            Modifier::Alt => &mut self.alt,
            Modifier::Logo => &mut self.logo,
        }
    }
}
