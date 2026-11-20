use rikka_app::prelude::{ColumnSpec, column_spec};
use rikka_i18n::tr;

pub(crate) const TILE: f32 = 116.0;
pub(crate) const TILE_ART: f32 = 84.0;
pub(crate) const TILE_LINE: f32 = 17.0;
pub(crate) const TILE_LINES: usize = 2;
pub(crate) const ROW: f32 = 28.0;

pub(crate) fn tile_height(lines: usize) -> f32 {
    TILE_ART + 4.0 + lines as f32 * TILE_LINE + 6.0
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Column {
    Name,
    Modified,
    Kind,
    Size,
}

impl Column {
    pub(crate) const ALL: [Self; 4] = [Self::Name, Self::Modified, Self::Kind, Self::Size];

    pub(crate) fn spec(self) -> ColumnSpec {
        let (heading, floor, ceiling) = match self {
            Self::Name => ("files.column.name", 120.0, 520.0),
            Self::Modified => ("files.column.modified", 96.0, 200.0),
            Self::Kind => ("files.column.kind", 72.0, 240.0),
            Self::Size => ("files.column.size", 64.0, 140.0),
        };

        column_spec(tr!(heading).to_string())
            .floor(floor)
            .ceiling(ceiling)
            .right_aligned(self.right_aligned())
    }

    pub(crate) const fn floor(self) -> f32 {
        match self {
            Self::Name => 120.0,
            Self::Modified => 96.0,
            Self::Kind => 72.0,
            Self::Size => 64.0,
        }
    }

    pub(crate) const fn right_aligned(self) -> bool {
        matches!(self, Self::Size)
    }

    pub(crate) const fn sort_key(self) -> files_core::SortKey {
        match self {
            Self::Name => files_core::SortKey::Name,
            Self::Modified => files_core::SortKey::Modified,
            Self::Kind => files_core::SortKey::Kind,
            Self::Size => files_core::SortKey::Size,
        }
    }
}
