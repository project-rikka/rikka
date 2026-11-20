use rikka_settings::seam::Settings;
use rikka_settings::session::remote;

const MEGABIT: u64 = 1_000_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Options {
    pub(crate) enabled: bool,
    pub(crate) listening: Listening,
    pub(crate) media: MediaOptions,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Listening {
    pub(crate) port: u16,
    pub(crate) network: bool,
    pub(crate) tailscale: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MediaOptions {
    pub(crate) max_bitrate: u64,
    pub(crate) sound: bool,
    pub(crate) display: Option<String>,
}

impl Options {
    pub(crate) fn read(settings: &Settings) -> Options {
        let port = settings.read_or(remote::PORT, remote::PORT_DEFAULT);
        let max_bitrate = settings.read_or(remote::MAX_BITRATE, remote::MAX_BITRATE_DEFAULT);
        let display: String = settings.read_or(remote::DISPLAY, remote::FIRST_DISPLAY.to_owned());
        Options {
            enabled: settings.read_or(remote::ENABLED, remote::ENABLED_DEFAULT),
            listening: Listening {
                port: u16::try_from(port.clamp(*remote::PORTS.start(), *remote::PORTS.end()))
                    .unwrap_or(u16::MAX),
                network: settings.read_or(remote::NETWORK, remote::NETWORK_DEFAULT),
                tailscale: settings.read_or(remote::TAILSCALE, remote::TAILSCALE_DEFAULT),
            },
            media: MediaOptions {
                max_bitrate: max_bitrate
                    .clamp(*remote::MAX_BITRATES.start(), *remote::MAX_BITRATES.end())
                    as u64
                    * MEGABIT,
                sound: settings.read_or(remote::SOUND, remote::SOUND_DEFAULT),
                display: (!display.is_empty()).then_some(display),
            },
        }
    }
}
