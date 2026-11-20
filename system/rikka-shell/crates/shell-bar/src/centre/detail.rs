use rikka_session::settings_app::{Link, Section, Spot};
use rikka_ui::{Label, tr};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Detail {
    Sound,
    Microphone,
    Network,
    Bluetooth,
}

impl Detail {
    #[must_use]
    pub(crate) fn title(self) -> Label {
        tr!(match self {
            Detail::Sound => "shell.centre.sound",
            Detail::Microphone => "shell.centre.microphone",
            Detail::Network => "shell.centre.wifi",
            Detail::Bluetooth => "shell.centre.bluetooth",
        })
    }

    #[must_use]
    pub(crate) fn settings(self) -> Label {
        tr!(match self {
            Detail::Sound | Detail::Microphone => "shell.centre.sound-settings",
            Detail::Network => "shell.centre.network-settings",
            Detail::Bluetooth => "shell.centre.bluetooth-settings",
        })
    }

    #[must_use]
    pub(crate) fn link(self) -> Link {
        match self {
            Detail::Sound => Link::Spot(Spot::SoundOutput),
            Detail::Microphone => Link::Spot(Spot::SoundInput),
            Detail::Network => Link::Section(Section::Network),
            Detail::Bluetooth => Link::Section(Section::Bluetooth),
        }
    }
}
