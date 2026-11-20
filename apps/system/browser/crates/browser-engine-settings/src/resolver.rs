#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Resolver {
    Cloudflare,
    NextDns,
    Quad9,
    Google,
}

impl Resolver {
    pub const ALL: [Resolver; 4] = [
        Resolver::Cloudflare,
        Resolver::NextDns,
        Resolver::Quad9,
        Resolver::Google,
    ];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Resolver::Cloudflare => "Cloudflare",
            Resolver::NextDns => "NextDNS",
            Resolver::Quad9 => "Quad9",
            Resolver::Google => "Google",
        }
    }

    #[must_use]
    pub fn uri(self) -> &'static str {
        match self {
            Resolver::Cloudflare => "https://mozilla.cloudflare-dns.com/dns-query",
            Resolver::NextDns => "https://firefox.dns.nextdns.io/",
            Resolver::Quad9 => "https://dns.quad9.net/dns-query",
            Resolver::Google => "https://dns.google/dns-query",
        }
    }

    #[must_use]
    pub fn of(uri: &str) -> Option<Resolver> {
        Resolver::ALL
            .into_iter()
            .find(|resolver| resolver.uri() == uri)
    }
}
