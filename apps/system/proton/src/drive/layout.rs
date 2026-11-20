use proton_files::SortKey;
use rikka_app::prelude::{ColumnSpec, column_spec};
use rikka_i18n::tr;

pub(crate) const TILE: f32 = 116.0;
pub(crate) const TILE_ART: f32 = 84.0;
pub(crate) const TILE_LINE: f32 = 17.0;
pub(crate) const TILE_LINES: usize = 2;
pub(crate) const ROW: f32 = 28.0;
pub(crate) const GUTTER: f32 = 26.0;

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
    pub(crate) const ALL: [Column; 4] =
        [Column::Name, Column::Modified, Column::Kind, Column::Size];

    pub(crate) fn spec(self) -> ColumnSpec {
        let (heading, floor, ceiling) = match self {
            Column::Name => ("proton.drive.column-name", 120.0, 520.0),
            Column::Modified => ("proton.drive.column-modified", 96.0, 200.0),
            Column::Kind => ("proton.drive.column-kind", 72.0, 240.0),
            Column::Size => ("proton.drive.column-size", 64.0, 140.0),
        };

        column_spec(tr!(heading).to_string())
            .floor(floor)
            .ceiling(ceiling)
            .right_aligned(self.right_aligned())
    }

    pub(crate) const fn right_aligned(self) -> bool {
        matches!(self, Column::Size)
    }

    pub(crate) const fn sorted(self) -> SortKey {
        match self {
            Column::Name => SortKey::Name,
            Column::Modified => SortKey::Modified,
            Column::Kind => SortKey::Kind,
            Column::Size => SortKey::Size,
        }
    }
}
