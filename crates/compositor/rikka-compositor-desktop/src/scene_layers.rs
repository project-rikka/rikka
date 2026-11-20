#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SceneLayers {
    pub top: bool,
    pub windows: bool,
    pub backdrop: bool,
}

impl SceneLayers {
    pub const ALL: Self = Self {
        top: true,
        windows: true,
        backdrop: true,
    };

    pub const WITHOUT_WINDOWS: Self = Self {
        windows: false,
        ..Self::ALL
    };
}
