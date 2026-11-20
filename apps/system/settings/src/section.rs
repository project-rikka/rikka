use rikka_app::prelude::*;
pub(crate) use rikka_session::settings_app::Section;

use crate::hardware::{Hardware, Radio};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Group {
    Personal,
    Devices,
    Network,
    System,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Needs {
    Nothing,
    Radio(Radio),
    Draft,
    Nixos,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Entry {
    Heading(Group),
    Item(Section),
}

struct About {
    group: Group,
    symbol: MaterialSymbol,
    needs: Needs,
}

const fn about(section: Section) -> About {
    let (group, symbol, needs) = match section {
        Section::Users => (
            Group::Personal,
            MaterialSymbol::ACCOUNT_CIRCLE,
            Needs::Draft,
        ),
        Section::Appearance => (Group::Personal, MaterialSymbol::PALETTE, Needs::Nothing),
        Section::Windows => (Group::Personal, MaterialSymbol::WINDOW, Needs::Nothing),
        Section::Notifications => (
            Group::Personal,
            MaterialSymbol::NOTIFICATIONS,
            Needs::Nothing,
        ),
        Section::Lock => (Group::Personal, MaterialSymbol::LOCK, Needs::Nothing),
        Section::Keyring => (Group::Personal, MaterialSymbol::KEY, Needs::Nothing),
        Section::Capture => (
            Group::Personal,
            MaterialSymbol::SCREENSHOT_MONITOR,
            Needs::Nothing,
        ),
        Section::Replay => (Group::Personal, MaterialSymbol::REPLAY, Needs::Nothing),
        Section::ColorPicker => (Group::Personal, MaterialSymbol::COLORIZE, Needs::Nothing),
        Section::Displays => (
            Group::Devices,
            MaterialSymbol::DESKTOP_WINDOWS,
            Needs::Nothing,
        ),
        Section::Pointer => (Group::Devices, MaterialSymbol::MOUSE, Needs::Nothing),
        Section::Keyboard => (Group::Devices, MaterialSymbol::KEYBOARD, Needs::Draft),
        Section::Audio => (Group::Devices, MaterialSymbol::VOLUME_UP, Needs::Nothing),
        Section::Power => (
            Group::Devices,
            MaterialSymbol::BATTERY_CHARGING_FULL,
            Needs::Draft,
        ),
        Section::Network => (
            Group::Network,
            MaterialSymbol::WIFI,
            Needs::Radio(Radio::Wireless),
        ),
        Section::Bluetooth => (
            Group::Network,
            MaterialSymbol::BLUETOOTH,
            Needs::Radio(Radio::Bluetooth),
        ),
        Section::Remote => (Group::Network, MaterialSymbol::SCREEN_SHARE, Needs::Draft),
        Section::Language => (Group::System, MaterialSymbol::LANGUAGE, Needs::Draft),
        Section::DateTime => (Group::System, MaterialSymbol::SCHEDULE, Needs::Draft),
        Section::Defaults => (Group::System, MaterialSymbol::APPS, Needs::Nothing),
        Section::Llm => (Group::System, MaterialSymbol::AUTO_AWESOME, Needs::Nothing),
        Section::System => (Group::System, MaterialSymbol::COMPUTER, Needs::Nixos),
    };
    About {
        group,
        symbol,
        needs,
    }
}

impl Group {
    const ALL: [Self; 4] = [Self::Personal, Self::Devices, Self::Network, Self::System];

    fn title(self) -> Label {
        match self {
            Self::Personal => tr!("settings.group.personal"),
            Self::Devices => tr!("settings.group.devices"),
            Self::Network => tr!("settings.group.network"),
            Self::System => tr!("settings.group.system"),
        }
    }
}

impl Entry {
    pub(crate) fn nav_item(self) -> NavItem {
        match self {
            Self::Heading(group) => nav_header(group.title()),
            Self::Item(section) => nav_item(title(section)).icon(about(section).symbol),
        }
    }
}

pub(crate) fn available(hardware: Hardware) -> impl Iterator<Item = Section> {
    Section::ALL
        .into_iter()
        .filter(move |section| match about(*section).needs {
            Needs::Radio(radio) => hardware.has(radio),
            Needs::Nothing | Needs::Draft | Needs::Nixos => true,
        })
}

pub(crate) fn entries(hardware: Hardware) -> Vec<Entry> {
    let mut entries = Vec::new();
    for group in Group::ALL {
        let mut members = available(hardware)
            .filter(|section| about(*section).group == group)
            .peekable();
        if members.peek().is_some() {
            entries.push(Entry::Heading(group));
        }
        entries.extend(members.map(Entry::Item));
    }
    entries
}

pub(crate) fn title(section: Section) -> Label {
    Label::translated(&format!("settings.section.{}", section.slug()))
}

pub(crate) fn needs(section: Section) -> Needs {
    about(section).needs
}

pub(crate) fn drafts(section: Section) -> bool {
    matches!(needs(section), Needs::Draft | Needs::Nixos)
}
