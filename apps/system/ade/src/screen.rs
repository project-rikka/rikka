#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Screen {
    #[default]
    Work,
    Settings,
}

impl Screen {
    pub(crate) fn at(self, project: bool) -> usize {
        match self {
            Screen::Work => usize::from(project),
            Screen::Settings => 2,
        }
    }
}
