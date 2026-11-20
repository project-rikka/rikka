pub(crate) const PATH: &str = "/org/mpris/MediaPlayer2";

pub(crate) const ROOT: &str = "org.mpris.MediaPlayer2";

pub(crate) const PLAYER: &str = "org.mpris.MediaPlayer2.Player";

pub(crate) const PREFIX: &str = "org.mpris.MediaPlayer2.";

pub(crate) const PROPERTIES: &str = "org.freedesktop.DBus.Properties";

pub(crate) const PROXIES: [&str; 1] = ["playerctld"];

pub(crate) fn bus_name(name: &str) -> String {
    let element: String = name
        .chars()
        .map(|c| match c.is_ascii_alphanumeric() || c == '_' {
            true => c,
            false => '_',
        })
        .collect();
    match element.starts_with(|c: char| c.is_ascii_digit()) || element.is_empty() {
        true => format!("{PREFIX}_{element}"),
        false => format!("{PREFIX}{element}"),
    }
}
