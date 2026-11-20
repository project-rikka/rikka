use rikka_compositor_desktop::Placement;
use rikka_compositor_geometry::{Logical, Rect};
use rikka_compositor_shell::ToplevelStates;

use super::enlargement::Enlargements;

const TILED: ToplevelStates = ToplevelStates::TILED_LEFT
    .union(ToplevelStates::TILED_RIGHT)
    .union(ToplevelStates::TILED_TOP)
    .union(ToplevelStates::TILED_BOTTOM);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Snapped {
    pub(crate) restore: Rect<i32, Logical>,
    pub(crate) zone: Rect<i32, Logical>,
    pub(crate) area: Rect<i32, Logical>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Enlarged {
    pub(crate) restore: Rect<i32, Logical>,
    pub(crate) modes: Enlargements,
    pub(crate) snapped: Option<Snapped>,
}

impl Enlarged {
    pub(crate) fn shrunk(self, tiled: bool) -> Mode {
        match (tiled, self.snapped) {
            (true, _) => Mode::Free {
                restore: Some(self.restore),
            },
            (false, Some(snapped)) => Mode::Snapped(snapped),
            (false, None) => Mode::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    Free { restore: Option<Rect<i32, Logical>> },
    Snapped(Snapped),
    Enlarged(Enlarged),
}

impl Default for Mode {
    fn default() -> Self {
        Self::Free { restore: None }
    }
}

impl Mode {
    pub(crate) fn enlarged(self) -> Option<Enlarged> {
        match self {
            Self::Enlarged(enlarged) => Some(enlarged),
            Self::Free { .. } | Self::Snapped(_) => None,
        }
    }

    pub(crate) fn enlargements(self) -> Enlargements {
        self.enlarged()
            .map_or_else(Enlargements::default, |enlarged| enlarged.modes)
    }

    pub(crate) fn snapped(self) -> Option<Snapped> {
        match self {
            Self::Snapped(snapped) => Some(snapped),
            Self::Enlarged(enlarged) => enlarged.snapped,
            Self::Free { .. } => None,
        }
    }

    pub(crate) fn snapped_mut(&mut self) -> Option<&mut Snapped> {
        match self {
            Self::Snapped(snapped) => Some(snapped),
            Self::Enlarged(enlarged) => enlarged.snapped.as_mut(),
            Self::Free { .. } => None,
        }
    }

    pub(crate) fn is_free(self) -> bool {
        matches!(self, Self::Free { .. })
    }

    pub(crate) fn enlarged_to(
        self,
        modes: Enlargements,
        standing: Rect<i32, Logical>,
        tiled: bool,
    ) -> Self {
        let (restore, snapped) = match self {
            Self::Free { restore } => (restore.filter(|_| tiled).unwrap_or(standing), None),
            Self::Snapped(snapped) => (standing, Some(snapped)),
            Self::Enlarged(enlarged) => (enlarged.restore, enlarged.snapped),
        };
        Self::Enlarged(Enlarged {
            restore,
            modes,
            snapped,
        })
    }

    pub(crate) fn snapped_into(
        self,
        standing: Rect<i32, Logical>,
        zone: Rect<i32, Logical>,
        area: Rect<i32, Logical>,
    ) -> Self {
        let restore = match self {
            Self::Free { .. } => standing,
            Self::Snapped(snapped) => snapped.restore,
            Self::Enlarged(enlarged) => enlarged
                .snapped
                .map_or(enlarged.restore, |snapped| snapped.restore),
        };
        Self::Snapped(Snapped {
            restore,
            zone,
            area,
        })
    }

    pub(crate) fn unsnap(&mut self) -> Option<Snapped> {
        match self {
            Self::Snapped(snapped) => {
                let snapped = *snapped;
                *self = Self::default();
                Some(snapped)
            }
            Self::Enlarged(enlarged) => enlarged.snapped.take(),
            Self::Free { .. } => None,
        }
    }

    pub(crate) fn tiled_from(&mut self, standing: Rect<i32, Logical>) {
        if let Self::Free { restore } = self {
            *restore = Some(standing);
        }
    }

    pub(crate) fn untiled(&mut self) -> Option<Rect<i32, Logical>> {
        match self {
            Self::Free { restore } => restore.take(),
            Self::Snapped(_) | Self::Enlarged(_) => None,
        }
    }

    pub(crate) fn placement(self, tiled: bool) -> Placement {
        placement(self.enlargements(), tiled)
    }

    pub(crate) fn states(self, tiled: bool) -> ToplevelStates {
        let placement = self.placement(tiled);
        states(placement, self.enlargements(), self.snapped().is_some())
    }
}

pub(crate) fn placement(enlarged: Enlargements, tiled: bool) -> Placement {
    match (enlarged.fullscreen, enlarged.maximized, tiled) {
        (true, _, _) => Placement::Fullscreen,
        (false, true, _) => Placement::Maximized,
        (false, false, true) => Placement::Tiled,
        (false, false, false) => Placement::Floating,
    }
}

pub(crate) fn states(
    placement: Placement,
    enlarged: Enlargements,
    snapped: bool,
) -> ToplevelStates {
    match placement {
        Placement::Tiled => TILED,
        Placement::Maximized | Placement::Fullscreen => enlarged.states(),
        Placement::Floating if snapped => TILED,
        Placement::Floating => ToplevelStates::empty(),
    }
}
