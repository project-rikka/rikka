pub const BUS_ADDRESS_ENV: &str = "DBUS_SESSION_BUS_ADDRESS";

pub const SESSION_VARIABLES: [&str; 6] = [
    "WAYLAND_DISPLAY",
    "WAYLAND_SOCKET",
    "DISPLAY",
    "XMODIFIERS",
    rikka_seam::net::SOCKET_ENV,
    BUS_ADDRESS_ENV,
];
