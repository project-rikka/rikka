use rikka_app::prelude::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum Section {
    #[default]
    Llm,
    Servers,
    Skills,
    Commands,
}

impl Section {
    pub(crate) const ALL: [Section; 4] = [
        Section::Llm,
        Section::Servers,
        Section::Skills,
        Section::Commands,
    ];

    pub(crate) fn title(self) -> Label {
        match self {
            Section::Llm => tr!("ade.settings.llm"),
            Section::Servers => tr!("ade.settings.servers.title"),
            Section::Skills => tr!("ade.settings.skills.title"),
            Section::Commands => tr!("ade.settings.commands.title"),
        }
    }

    fn symbol(self) -> MaterialSymbol {
        match self {
            Section::Llm => MaterialSymbol::AUTO_AWESOME,
            Section::Servers => MaterialSymbol::EXTENSION,
            Section::Skills => MaterialSymbol::SCHOOL,
            Section::Commands => MaterialSymbol::KEYBOARD_COMMAND_KEY,
        }
    }

    pub(crate) fn nav_item(self) -> NavItem {
        nav_item(self.title()).icon(self.symbol())
    }
}
