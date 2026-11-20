use crate::key::Key;
use crate::schema::Schema;

pub const DOMAIN: &str = "rikka.network";

pub const MODE: &str = "proxy-mode";
pub const AUTOCONFIGURATION: &str = "proxy-autoconfiguration";
pub const SAME_FOR_ALL: &str = "proxy-same-for-all";
pub const IGNORED: &str = "proxy-ignored";

pub const NONE: &str = "none";
pub const MANUAL: &str = "manual";
pub const AUTOMATIC: &str = "automatic";

pub const MODES: [&str; 3] = [NONE, MANUAL, AUTOMATIC];

pub const SAME_FOR_ALL_DEFAULT: bool = true;
pub const IGNORED_DEFAULT: &str = "localhost, 127.0.0.0/8, ::1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Proxy {
    pub host: &'static str,
    pub port: &'static str,
}

pub const HTTP: Proxy = Proxy {
    host: "proxy-http-host",
    port: "proxy-http-port",
};
pub const HTTPS: Proxy = Proxy {
    host: "proxy-https-host",
    port: "proxy-https-port",
};
pub const FTP: Proxy = Proxy {
    host: "proxy-ftp-host",
    port: "proxy-ftp-port",
};
pub const SOCKS: Proxy = Proxy {
    host: "proxy-socks-host",
    port: "proxy-socks-port",
};

pub const PROXIES: [Proxy; 4] = [HTTP, HTTPS, FTP, SOCKS];

#[must_use]
pub fn schema() -> Schema {
    let mut schema = Schema::new(DOMAIN)
        .summary("Network proxy for session programs")
        .key(
            Key::string(MODE, NONE)
                .summary("Proxy mode: none, manual or automatic")
                .choices(MODES),
        )
        .key(
            Key::string(AUTOCONFIGURATION, "")
                .summary("Proxy auto-configuration (PAC) URL for automatic mode"),
        )
        .key(
            Key::bool(SAME_FOR_ALL, SAME_FOR_ALL_DEFAULT)
                .summary("Use the HTTP proxy for all protocols"),
        )
        .key(Key::string(IGNORED, IGNORED_DEFAULT).summary(
            "Comma-separated hosts to bypass: names, suffixes, addresses and CIDR blocks",
        ));
    for proxy in PROXIES {
        schema = schema
            .key(Key::string(proxy.host, "").summary("Proxy host"))
            .key(
                Key::int(proxy.port, 0)
                    .summary("Proxy port; zero for none")
                    .range(0..=65_535),
            );
    }
    schema
}
