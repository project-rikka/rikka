use files_core::{Entry, Family, Location};
use rikka_format::format_bytes;
use rikka_ui::MaterialSymbol;

#[derive(Clone, Debug)]
pub(crate) struct Row {
    pub(crate) at: Location,
    pub(crate) name: String,
    pub(crate) modified: String,
    pub(crate) kind: String,
    pub(crate) size: String,
    pub(crate) family: Family,
    pub(crate) linked: bool,
    pub(crate) broken: bool,
    pub(crate) hidden: bool,
}

impl Row {
    pub(crate) fn symbol(&self) -> MaterialSymbol {
        if self.broken {
            return MaterialSymbol::LINK_OFF;
        }
        match self.family {
            Family::Folder => MaterialSymbol::FOLDER,
            Family::Image => MaterialSymbol::IMAGE,
            Family::Audio => MaterialSymbol::AUDIO_FILE,
            Family::Video => MaterialSymbol::MOVIE,
            Family::Document => MaterialSymbol::DESCRIPTION,
            Family::Text => MaterialSymbol::DRAFT,
            Family::Archive => MaterialSymbol::FOLDER_ZIP,
            Family::Program => MaterialSymbol::TERMINAL,
            Family::Font => MaterialSymbol::FONT_DOWNLOAD,
            Family::Other => MaterialSymbol::INSERT_DRIVE_FILE,
        }
    }

    pub(crate) fn is_directory(&self) -> bool {
        self.family == Family::Folder
    }

    pub(crate) fn cell(&self, column: super::layout::Column) -> &str {
        match column {
            super::layout::Column::Name => &self.name,
            super::layout::Column::Modified => &self.modified,
            super::layout::Column::Kind => &self.kind,
            super::layout::Column::Size => &self.size,
        }
    }

    pub(crate) fn build(entry: &Entry, kind: String) -> Self {
        Self {
            name: entry.name.clone(),
            modified: files_core::format::timestamp(entry.modified),
            kind,
            size: if entry.is_directory() {
                String::new()
            } else {
                format_bytes(entry.size)
            },
            family: entry.family(),
            linked: entry.link.is_some(),
            broken: entry.link.as_ref().is_some_and(|link| link.broken),
            hidden: entry.hidden,
            at: entry.location.clone(),
        }
    }
}
