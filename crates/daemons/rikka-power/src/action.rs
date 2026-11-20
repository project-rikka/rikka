#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    PowerOff,
    Reboot,
    Suspend,
}

impl Action {
    pub const ALL: [Action; 3] = [Action::PowerOff, Action::Reboot, Action::Suspend];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Ask(Action),
    Brighten,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PowerCapabilities {
    power_off: bool,
    reboot: bool,
    suspend: bool,
    hibernate: bool,
}

impl PowerCapabilities {
    #[must_use]
    pub fn allows(&self, action: Action) -> bool {
        match action {
            Action::PowerOff => self.power_off,
            Action::Reboot => self.reboot,
            Action::Suspend => self.suspend,
        }
    }

    #[must_use]
    pub fn hibernates(&self) -> bool {
        self.hibernate
    }

    pub(crate) fn allow(&mut self, action: Action, allowed: bool) {
        match action {
            Action::PowerOff => self.power_off = allowed,
            Action::Reboot => self.reboot = allowed,
            Action::Suspend => self.suspend = allowed,
        }
    }

    pub(crate) fn allow_hibernate(&mut self, allowed: bool) {
        self.hibernate = allowed;
    }
}

pub(crate) fn permits(answer: &str) -> bool {
    matches!(answer, "yes" | "challenge")
}
