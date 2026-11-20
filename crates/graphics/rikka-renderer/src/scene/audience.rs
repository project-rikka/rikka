#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Viewer {
    #[default]
    Screen,
    Capture,
}

impl Viewer {
    pub(crate) const ALL: [Viewer; 2] = [Viewer::Screen, Viewer::Capture];

    pub(crate) fn index(self) -> usize {
        match self {
            Viewer::Screen => 0,
            Viewer::Capture => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Audience {
    #[default]
    Everyone,
    Only(Viewer),
}

impl Audience {
    #[must_use]
    pub fn admits(self, viewer: Viewer) -> bool {
        match self {
            Audience::Everyone => true,
            Audience::Only(only) => only == viewer,
        }
    }
}
