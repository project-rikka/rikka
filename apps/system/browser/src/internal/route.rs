use browser_pages::{Internal, Section};
use rikka_app::prelude::*;

pub(crate) trait Presented {
    fn title(&self) -> Label;
    fn symbol(&self) -> MaterialSymbol;
}

impl Presented for Internal {
    fn title(&self) -> Label {
        match self {
            Internal::Home => tr!("browser.home.title"),
            Internal::Settings(Some(Section::ClearData)) => tr!("browser.clear.title"),
            Internal::Settings(_) => tr!("browser.settings.title"),
            Internal::History => tr!("browser.history.title"),
            Internal::Archive => tr!("browser.archive.title"),
            Internal::Downloads => tr!("browser.downloads.title"),
            Internal::Extensions => tr!("browser.extensions.title"),
            Internal::Source(_) => tr!("browser.source.title"),
            Internal::Missing(_) => tr!("browser.missing.title"),
        }
    }

    fn symbol(&self) -> MaterialSymbol {
        match self {
            Internal::Home => MaterialSymbol::HOME,
            Internal::Settings(_) => MaterialSymbol::SETTINGS,
            Internal::History => MaterialSymbol::HISTORY,
            Internal::Archive => MaterialSymbol::INVENTORY_2,
            Internal::Downloads => MaterialSymbol::DOWNLOAD,
            Internal::Extensions => MaterialSymbol::EXTENSION,
            Internal::Source(_) => MaterialSymbol::CODE,
            Internal::Missing(_) => MaterialSymbol::WARNING,
        }
    }
}
