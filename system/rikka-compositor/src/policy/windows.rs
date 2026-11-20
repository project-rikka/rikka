use rikka_compositor_tiling::Gaps;
use rikka_session::profile::Profile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LayoutMode {
    Floating,
    Tiling,
    Scrolling,
}

const LAYOUTS: [(LayoutMode, &str); 2] = [
    (LayoutMode::Floating, "floating"),
    (LayoutMode::Tiling, "tiling"),
];

impl LayoutMode {
    pub(crate) fn from_name(name: &str, scrolling: bool) -> Option<Self> {
        let mode = LAYOUTS
            .iter()
            .find(|(_, label)| *label == name)
            .map(|(mode, _)| *mode)?;
        Some(match (mode, scrolling) {
            (Self::Tiling, true) => Self::Scrolling,
            (mode, _) => mode,
        })
    }

    pub(crate) fn tiles(self) -> bool {
        self != Self::Floating
    }

    pub(crate) fn focus_follows_pointer(self) -> bool {
        self == Self::Tiling
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SnapRules {
    pub(crate) windows: bool,
    pub(crate) bar: bool,
    pub(crate) assist: bool,
    pub(crate) resize_together: bool,
}

impl SnapRules {
    pub(crate) fn declared() -> Self {
        Self {
            windows: true,
            bar: true,
            assist: true,
            resize_together: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WindowRules {
    pub(crate) mode: LayoutMode,
    pub(crate) gaps: Gaps,
    pub(crate) resize_on_border: bool,
    pub(crate) focus_outline: bool,
    pub(crate) snap: SnapRules,
}

impl WindowRules {
    pub(crate) fn declared() -> Self {
        Self {
            mode: LayoutMode::Floating,
            gaps: Gaps { inner: 8, outer: 8 },
            resize_on_border: true,
            focus_outline: true,
            snap: SnapRules::declared(),
        }
    }

    #[must_use]
    pub(crate) fn for_profile(self, profile: Profile) -> WindowRules {
        match profile {
            Profile::Full => self,
            Profile::Simple => WindowRules {
                mode: LayoutMode::Floating,
                ..self
            },
        }
    }
}
