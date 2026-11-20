use browser_engine_settings::{
    EngineSettings, InputError, Resolver, parse_bypass, parse_endpoint, parse_resolver,
    parse_script,
};
use rikka_app::prelude::*;
use rikka_web::{DnsOverHttps, Endpoint, ManualProxy, Proxy};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum ProxyMode {
    #[default]
    System,
    Direct,
    Detect,
    Script,
    Manual,
}

impl ProxyMode {
    pub(crate) const ALL: [ProxyMode; 5] = [
        ProxyMode::System,
        ProxyMode::Direct,
        ProxyMode::Detect,
        ProxyMode::Script,
        ProxyMode::Manual,
    ];

    fn of(proxy: &Proxy) -> ProxyMode {
        match proxy {
            Proxy::System => ProxyMode::System,
            Proxy::Direct => ProxyMode::Direct,
            Proxy::Detect => ProxyMode::Detect,
            Proxy::Script(_) => ProxyMode::Script,
            Proxy::Manual(_) => ProxyMode::Manual,
        }
    }

    pub(crate) fn immediate(self) -> Option<Proxy> {
        match self {
            ProxyMode::System => Some(Proxy::System),
            ProxyMode::Direct => Some(Proxy::Direct),
            ProxyMode::Detect => Some(Proxy::Detect),
            ProxyMode::Script | ProxyMode::Manual => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum DnsMode {
    #[default]
    Off,
    Preferred,
    Only,
}

impl DnsMode {
    pub(crate) const ALL: [DnsMode; 3] = [DnsMode::Off, DnsMode::Preferred, DnsMode::Only];

    pub(crate) fn of(dns: &DnsOverHttps) -> DnsMode {
        match dns {
            DnsOverHttps::Off => DnsMode::Off,
            DnsOverHttps::Preferred(_) => DnsMode::Preferred,
            DnsOverHttps::Only(_) => DnsMode::Only,
        }
    }

    pub(crate) fn with(self, uri: String) -> DnsOverHttps {
        match self {
            DnsMode::Off => DnsOverHttps::Off,
            DnsMode::Preferred => DnsOverHttps::Preferred(uri),
            DnsMode::Only => DnsOverHttps::Only(uri),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Input {
    Resolver,
    Script,
    Http,
    Https,
    Socks,
}

#[derive(State)]
pub(crate) struct EngineForms {
    #[untracked]
    pub(crate) resolver: TextDocument,
    #[untracked]
    pub(crate) script: TextDocument,
    #[untracked]
    pub(crate) http: TextDocument,
    #[untracked]
    pub(crate) https: TextDocument,
    #[untracked]
    pub(crate) socks: TextDocument,
    #[untracked]
    pub(crate) bypass: TextDocument,
    #[untracked]
    pub(crate) style: TextDocument,
    remote_dns: Tracked<bool>,
    custom_resolver: Tracked<bool>,
    proxy_mode: Tracked<ProxyMode>,
    problem: Tracked<Option<(Input, InputError)>>,
}

fn endpoint_text(endpoint: Option<&Endpoint>) -> String {
    match endpoint {
        Some(endpoint) if endpoint.host.contains(':') => {
            format!("[{}]:{}", endpoint.host, endpoint.port)
        }
        Some(endpoint) => format!("{}:{}", endpoint.host, endpoint.port),
        None => String::new(),
    }
}

impl EngineForms {
    pub(crate) fn new() -> EngineForms {
        EngineForms {
            resolver: TextDocument::new(""),
            script: TextDocument::new(""),
            http: TextDocument::new(""),
            https: TextDocument::new(""),
            socks: TextDocument::new(""),
            bypass: TextDocument::new(""),
            style: TextDocument::new(""),
            remote_dns: Tracked::new(false),
            custom_resolver: Tracked::new(false),
            proxy_mode: Tracked::new(ProxyMode::System),
            problem: Tracked::new(None),
        }
    }

    pub(crate) fn reset(&mut self, settings: &EngineSettings) {
        let uri = match &settings.dns_over_https {
            DnsOverHttps::Off => None,
            DnsOverHttps::Preferred(uri) | DnsOverHttps::Only(uri) => Some(uri.as_str()),
        };
        let custom = uri.filter(|uri| Resolver::of(uri).is_none());
        self.resolver.reset(custom.unwrap_or_default());
        self.custom_resolver.set(custom.is_some());
        let manual = match &settings.proxy {
            Proxy::Manual(manual) => manual.clone(),
            _ => ManualProxy::default(),
        };
        let script = match &settings.proxy {
            Proxy::Script(script) => script.as_str(),
            _ => "",
        };
        self.script.reset(script);
        self.http.reset(endpoint_text(manual.http.as_ref()));
        self.https.reset(endpoint_text(manual.https.as_ref()));
        self.socks.reset(endpoint_text(manual.socks.as_ref()));
        self.bypass.reset(manual.bypass.join(", "));
        self.remote_dns.set(manual.socks_resolves_names);
        self.proxy_mode.set(ProxyMode::of(&settings.proxy));
        self.style.reset(&settings.user_style);
        self.problem.set(None);
    }

    pub(crate) fn proxy_mode(&self) -> ProxyMode {
        *self.proxy_mode
    }

    pub(crate) fn custom_resolver(&self) -> bool {
        *self.custom_resolver
    }

    pub(crate) fn remote_dns(&self) -> bool {
        *self.remote_dns
    }

    pub(crate) fn problem(&self, input: Input) -> Option<InputError> {
        (*self.problem)
            .filter(|(failed, _)| *failed == input)
            .map(|(_, error)| error)
    }

    pub(crate) fn choose_proxy_mode(&mut self, mode: ProxyMode) {
        self.proxy_mode.set(mode);
        self.problem.set(None);
    }

    pub(crate) fn choose_custom_resolver(&mut self, custom: bool) {
        self.custom_resolver.set(custom);
        self.problem.set(None);
    }

    pub(crate) fn set_remote_dns(&mut self, on: bool) {
        self.remote_dns.set(on);
    }

    fn failed<T>(&mut self, input: Input, parsed: Result<T, InputError>) -> Option<T> {
        match parsed {
            Ok(value) => {
                self.problem.set(None);
                Some(value)
            }
            Err(error) => {
                self.problem.set(Some((input, error)));
                None
            }
        }
    }

    pub(crate) fn resolver_uri(&mut self) -> Option<String> {
        let text = self.resolver.text();
        self.failed(Input::Resolver, parse_resolver(&text))
    }

    pub(crate) fn proxy(&mut self) -> Option<Proxy> {
        match *self.proxy_mode {
            ProxyMode::Script => {
                let text = self.script.text();
                self.failed(Input::Script, parse_script(&text))
                    .map(Proxy::Script)
            }
            ProxyMode::Manual => {
                let (http, https, socks) = (self.http.text(), self.https.text(), self.socks.text());
                let http = self.failed(Input::Http, parse_endpoint(&http))?;
                let https = self.failed(Input::Https, parse_endpoint(&https))?;
                let socks = self.failed(Input::Socks, parse_endpoint(&socks))?;
                Some(Proxy::Manual(ManualProxy {
                    http,
                    https,
                    socks,
                    socks_resolves_names: *self.remote_dns,
                    bypass: parse_bypass(&self.bypass.text()),
                }))
            }
            mode => mode.immediate(),
        }
    }
}

impl std::fmt::Debug for EngineForms {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EngineForms")
            .field("proxy_mode", &*self.proxy_mode)
            .finish_non_exhaustive()
    }
}
