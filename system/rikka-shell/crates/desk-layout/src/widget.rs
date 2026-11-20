use rikka_types::Size as Extent;
use serde::{Deserialize, Serialize};

use crate::area::{GAP, Spot};
use crate::content::Content;

const CELL: f32 = 152.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WidgetId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    Clock,
    Calendar,
    Weather,
    Media,
    Monitor,
    Power,
    Note,
    Photos,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Size {
    Small,
    Medium,
    Large,
    ExtraLarge,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Widget {
    pub id: WidgetId,
    pub size: Size,
    #[serde(flatten)]
    pub spot: Spot,
    #[serde(flatten)]
    pub content: Content,
}

impl Kind {
    pub const ALL: [Kind; 8] = [
        Kind::Clock,
        Kind::Calendar,
        Kind::Weather,
        Kind::Media,
        Kind::Monitor,
        Kind::Power,
        Kind::Note,
        Kind::Photos,
    ];

    #[must_use]
    pub fn sizes(self) -> &'static [Size] {
        const EVERY: &[Size] = &[Size::Small, Size::Medium, Size::Large, Size::ExtraLarge];
        const UP_TO_LARGE: &[Size] = &[Size::Small, Size::Medium, Size::Large];
        const UP_TO_MEDIUM: &[Size] = &[Size::Small, Size::Medium];
        match self {
            Kind::Clock | Kind::Note | Kind::Photos => EVERY,
            Kind::Calendar | Kind::Weather | Kind::Media | Kind::Monitor => UP_TO_LARGE,
            Kind::Power => UP_TO_MEDIUM,
        }
    }

    #[must_use]
    pub fn default_size(self) -> Size {
        match self {
            Kind::Calendar | Kind::Power => Size::Small,
            Kind::Clock
            | Kind::Weather
            | Kind::Media
            | Kind::Monitor
            | Kind::Note
            | Kind::Photos => Size::Medium,
        }
    }

    #[must_use]
    pub fn fits(self, size: Size) -> bool {
        self.sizes().contains(&size)
    }
}

impl Size {
    #[must_use]
    pub fn extent(self) -> Extent {
        let (across, down) = match self {
            Size::Small => (1, 1),
            Size::Medium => (2, 1),
            Size::Large => (2, 2),
            Size::ExtraLarge => (4, 2),
        };
        Extent::new(span(across), span(down))
    }
}

fn span(cells: u8) -> f32 {
    let cells = f32::from(cells);
    cells * CELL + (cells - 1.0) * GAP
}

impl Widget {
    #[must_use]
    pub fn new(id: WidgetId, content: Content, spot: Spot) -> Widget {
        Widget {
            id,
            size: content.kind().default_size(),
            spot,
            content,
        }
    }

    #[must_use]
    pub fn kind(&self) -> Kind {
        self.content.kind()
    }

    #[must_use]
    pub fn extent(&self) -> Extent {
        self.size.extent()
    }

    pub(super) fn sanitized(mut self) -> Widget {
        if !self.kind().fits(self.size) {
            self.size = self.kind().default_size();
        }
        self.spot = self.spot.sanitized();
        self.content = self.content.sanitized();
        self
    }
}
