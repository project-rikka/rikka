use std::cmp::Ordering;

use google_drive::{File, Kind};
use rikka_app::prelude::*;
use rikka_format::{format_bytes, format_timestamp, natural_cmp};

const MILLIS: i64 = 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ViewMode {
    Details,
    Icons,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Column {
    Name,
    Modified,
    Kind,
    Size,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Sorting {
    pub(crate) column: Option<Column>,
    pub(crate) descending: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Row {
    pub(crate) file: File,
    pub(crate) modified: String,
    pub(crate) kind: String,
    pub(crate) size: String,
}

impl Column {
    pub(crate) const ALL: [Column; 4] =
        [Column::Name, Column::Modified, Column::Kind, Column::Size];

    pub(crate) fn title(self) -> Label {
        match self {
            Column::Name => tr!("google.drive.column.name"),
            Column::Modified => tr!("google.drive.column.modified"),
            Column::Kind => tr!("google.drive.column.kind"),
            Column::Size => tr!("google.drive.column.size"),
        }
    }

    pub(crate) fn spec(self) -> ColumnSpec {
        let ceiling = match self {
            Column::Name => 560.0,
            Column::Modified => 200.0,
            Column::Kind => 240.0,
            Column::Size => 140.0,
        };
        column_spec(self.title().to_string())
            .floor(self.floor())
            .ceiling(ceiling)
            .right_aligned(self.right_aligned())
    }

    pub(crate) const fn floor(self) -> f32 {
        match self {
            Column::Name => 140.0,
            Column::Modified => 96.0,
            Column::Kind => 80.0,
            Column::Size => 64.0,
        }
    }

    pub(crate) const fn right_aligned(self) -> bool {
        matches!(self, Column::Size)
    }
}

impl Sorting {
    pub(crate) fn toggled(self, column: Column) -> Sorting {
        Sorting {
            column: Some(column),
            descending: self.column == Some(column) && !self.descending,
        }
    }

    pub(crate) fn sort(self, rows: &mut [Row]) {
        let Some(column) = self.column else {
            return;
        };
        rows.sort_by(|left, right| {
            let folders = right.file.is_folder().cmp(&left.file.is_folder());
            let ordered = match column {
                Column::Name => Ordering::Equal,
                Column::Modified => left.file.modified_time.cmp(&right.file.modified_time),
                Column::Kind => natural_cmp(&left.kind, &right.kind),
                Column::Size => left.file.size.cmp(&right.file.size),
            };
            let ordered = ordered.then_with(|| natural_cmp(&left.file.name, &right.file.name));
            folders.then(match self.descending {
                true => ordered.reverse(),
                false => ordered,
            })
        });
    }
}

impl Row {
    pub(crate) fn new(file: File) -> Row {
        Row {
            modified: file
                .modified_time
                .map(|millis| format_timestamp(millis / MILLIS))
                .unwrap_or_default(),
            kind: kind_name(file.kind()).to_string(),
            size: match (file.size, file.is_folder() || file.is_native()) {
                (Some(size), false) => format_bytes(size),
                _ => String::new(),
            },
            file,
        }
    }

    pub(crate) fn cell(&self, column: Column) -> &str {
        match column {
            Column::Name => &self.file.name,
            Column::Modified => &self.modified,
            Column::Kind => &self.kind,
            Column::Size => &self.size,
        }
    }

    pub(crate) fn symbol(&self) -> MaterialSymbol {
        match self.file.kind() {
            Kind::Folder if self.file.shared => MaterialSymbol::FOLDER_SHARED,
            Kind::Folder => MaterialSymbol::FOLDER,
            Kind::Document => MaterialSymbol::ARTICLE,
            Kind::Spreadsheet => MaterialSymbol::TABLE,
            Kind::Presentation => MaterialSymbol::SLIDESHOW,
            Kind::Drawing => MaterialSymbol::DRAW,
            Kind::Form => MaterialSymbol::ASSIGNMENT,
            Kind::Site => MaterialSymbol::WEB,
            Kind::Image => MaterialSymbol::IMAGE,
            Kind::Video => MaterialSymbol::MOVIE,
            Kind::Audio => MaterialSymbol::AUDIO_FILE,
            Kind::Pdf => MaterialSymbol::PICTURE_AS_PDF,
            Kind::Archive => MaterialSymbol::FOLDER_ZIP,
            Kind::Text => MaterialSymbol::DRAFT,
            Kind::Other => MaterialSymbol::INSERT_DRIVE_FILE,
        }
    }

    pub(crate) fn tint(&self, palette: &Palette) -> Color {
        match self.file.kind() {
            Kind::Folder => Color::rgb(0.98, 0.78, 0.36),
            Kind::Document => Color::rgb(0.26, 0.52, 0.96),
            Kind::Spreadsheet => Color::rgb(0.20, 0.66, 0.33),
            Kind::Presentation => Color::rgb(0.96, 0.71, 0.0),
            Kind::Drawing | Kind::Pdf => Color::rgb(0.92, 0.26, 0.21),
            Kind::Form => Color::rgb(0.47, 0.33, 0.78),
            Kind::Image => palette.success,
            Kind::Video => palette.accent,
            Kind::Audio => palette.warning,
            Kind::Archive => Color::rgb(0.85, 0.71, 0.42),
            Kind::Site | Kind::Text | Kind::Other => palette.text_muted,
        }
    }
}

pub(crate) fn kind_name(kind: Kind) -> Label {
    match kind {
        Kind::Folder => tr!("google.drive.kind.folder"),
        Kind::Document => tr!("google.drive.kind.document"),
        Kind::Spreadsheet => tr!("google.drive.kind.spreadsheet"),
        Kind::Presentation => tr!("google.drive.kind.presentation"),
        Kind::Drawing => tr!("google.drive.kind.drawing"),
        Kind::Form => tr!("google.drive.kind.form"),
        Kind::Site => tr!("google.drive.kind.site"),
        Kind::Image => tr!("google.drive.kind.image"),
        Kind::Video => tr!("google.drive.kind.video"),
        Kind::Audio => tr!("google.drive.kind.audio"),
        Kind::Pdf => tr!("google.drive.kind.pdf"),
        Kind::Archive => tr!("google.drive.kind.archive"),
        Kind::Text => tr!("google.drive.kind.text"),
        Kind::Other => tr!("google.drive.kind.file"),
    }
}
