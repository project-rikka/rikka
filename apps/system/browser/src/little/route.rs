use crate::command::Command;
use crate::launch::{Kind, Launch};
use browser_pages::{Internal, Section};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Routing {
    Here,
    Close,
    Browser(Internal),
    Ignore,
}

pub(crate) fn wants_little(launch: &Launch) -> bool {
    launch.kind == Kind::Main
        && !launch.addresses.is_empty()
        && launch
            .addresses
            .iter()
            .all(|address| Internal::parse(address).is_none())
}

pub(crate) fn routing(command: Command) -> Routing {
    match command {
        Command::CloseTab | Command::CloseWindow => Routing::Close,
        Command::Settings => Routing::Browser(Internal::Settings(None)),
        Command::History => Routing::Browser(Internal::History),
        Command::ClearData => Routing::Browser(Internal::Settings(Some(Section::ClearData))),
        Command::Downloads => Routing::Browser(Internal::Downloads),
        Command::NewTab
        | Command::ReopenTab
        | Command::NextTab
        | Command::PreviousTab
        | Command::Tab(_)
        | Command::LastTab
        | Command::TogglePin
        | Command::ToggleSidebar
        | Command::PreviousSpace
        | Command::NextSpace
        | Command::ToggleSplit
        | Command::NextPane
        | Command::PreviousPane
        | Command::FocusAddress
        | Command::Palette
        | Command::SearchAddress => Routing::Ignore,
        _ => Routing::Here,
    }
}
