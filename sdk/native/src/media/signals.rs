use rikka_ui::Observers;

#[derive(Debug, Default)]
pub(crate) struct Signals {
    state: Observers,
    position: Observers,
}

impl Signals {
    pub(crate) fn read_state(&self) {
        self.state.read();
    }

    pub(crate) fn read_position(&self) {
        self.position.read();
    }

    pub(crate) fn state_changed(&self) {
        self.state.changed();
    }

    pub(crate) fn position_changed(&self) {
        self.position.changed();
    }

    pub(crate) fn changed(&self) {
        self.state.changed();
        self.position.changed();
    }
}
