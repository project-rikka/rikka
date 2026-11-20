use rikka_compositor_shell::ToplevelStates;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Enlargement {
    Maximized,
    Fullscreen,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Enlargements {
    pub(crate) maximized: bool,
    pub(crate) fullscreen: bool,
}

impl Enlargements {
    pub(crate) fn with_mode(self, mode: Enlargement) -> Self {
        self.set(mode, true)
    }

    pub(crate) fn without_mode(self, mode: Enlargement) -> Self {
        self.set(mode, false)
    }

    fn set(self, mode: Enlargement, large: bool) -> Self {
        match mode {
            Enlargement::Maximized => Self {
                maximized: large,
                ..self
            },
            Enlargement::Fullscreen => Self {
                fullscreen: large,
                ..self
            },
        }
    }

    pub(crate) fn any(self) -> bool {
        self.maximized || self.fullscreen
    }

    pub(crate) fn states(self) -> ToplevelStates {
        let mut states = ToplevelStates::empty();
        states.set(ToplevelStates::MAXIMIZED, self.maximized);
        states.set(ToplevelStates::FULLSCREEN, self.fullscreen);
        states
    }
}
