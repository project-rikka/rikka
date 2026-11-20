use browser_engine_settings::{InputError, Resolver};
use rikka_app::prelude::*;
use rikka_web::DnsOverHttps;
use rikka_widgets::{column, row};

use crate::sections::engine::{CHOICE, change, current, title};
use crate::sections::forms::{DnsMode, Input, ProxyMode};
use crate::shell::Shell;

const ENDPOINT: f32 = 260.0;

fn dns_mode_name(mode: DnsMode) -> Label {
    match mode {
        DnsMode::Off => tr!("browser.network.dns-off"),
        DnsMode::Preferred => tr!("browser.network.dns-preferred"),
        DnsMode::Only => tr!("browser.network.dns-only"),
    }
}

fn proxy_mode_name(mode: ProxyMode) -> Label {
    match mode {
        ProxyMode::System => tr!("browser.network.proxy-system"),
        ProxyMode::Direct => tr!("browser.network.proxy-direct"),
        ProxyMode::Detect => tr!("browser.network.proxy-detect"),
        ProxyMode::Script => tr!("browser.network.proxy-script"),
        ProxyMode::Manual => tr!("browser.network.proxy-manual"),
    }
}

pub(crate) fn problem_name(error: InputError) -> Label {
    match error {
        InputError::MalformedUrl => tr!("browser.network.malformed-url"),
        InputError::NotHttps => tr!("browser.network.not-https"),
        InputError::UnsupportedScheme => tr!("browser.network.unsupported-scheme"),
        InputError::MissingHost => tr!("browser.network.missing-host"),
        InputError::InvalidPort => tr!("browser.network.invalid-port"),
    }
}

fn problem_line(error: Option<InputError>) -> Option<Element> {
    error.map(|error| {
        text(problem_name(error))
            .caption()
            .color(palette().danger)
            .any()
    })
}

impl Shell {
    fn choose_dns_mode(&mut self, mode: DnsMode) {
        let settings = current(self);
        let uri = match &settings.dns_over_https {
            DnsOverHttps::Preferred(uri) | DnsOverHttps::Only(uri) => Some(uri.clone()),
            DnsOverHttps::Off if self.sections.web.custom_resolver() => {
                match self.sections.web.resolver_uri() {
                    Some(uri) => Some(uri),
                    None => return,
                }
            }
            DnsOverHttps::Off => None,
        };
        let uri = uri.unwrap_or_else(|| Resolver::Cloudflare.uri().to_owned());
        change(self, |settings| settings.dns_over_https = mode.with(uri));
    }

    fn choose_resolver(&mut self, at: usize) {
        let mode = DnsMode::of(&current(self).dns_over_https);
        match Resolver::ALL.get(at) {
            Some(resolver) => {
                self.sections.web.choose_custom_resolver(false);
                let uri = resolver.uri().to_owned();
                change(self, |settings| settings.dns_over_https = mode.with(uri));
            }
            None => self.sections.web.choose_custom_resolver(true),
        }
    }

    fn apply_resolver(&mut self) {
        let mode = DnsMode::of(&current(self).dns_over_https);
        if let Some(uri) = self.sections.web.resolver_uri() {
            change(self, |settings| settings.dns_over_https = mode.with(uri));
        }
    }

    fn choose_proxy_mode(&mut self, mode: ProxyMode) {
        self.sections.web.choose_proxy_mode(mode);
        if let Some(proxy) = mode.immediate() {
            change(self, |settings| settings.proxy = proxy);
        }
    }

    fn apply_proxy(&mut self) {
        if let Some(proxy) = self.sections.web.proxy() {
            change(self, |settings| settings.proxy = proxy);
        }
    }
}

fn dns(shell: &Shell) -> Element {
    let dns = current(shell).dns_over_https;
    let mode = DnsMode::of(&dns);
    let forms = &shell.sections.web;
    let custom = forms.custom_resolver();
    let resolver = match &dns {
        DnsOverHttps::Preferred(uri) | DnsOverHttps::Only(uri) => Resolver::of(uri),
        DnsOverHttps::Off => None,
    };
    let chosen = match (custom, resolver) {
        (false, Some(resolver)) => Resolver::ALL.iter().position(|known| *known == resolver),
        (false, None) if mode == DnsMode::Off => Some(0),
        _ => Some(Resolver::ALL.len()),
    };
    let providers = (mode != DnsMode::Off).then(|| {
        field(
            tr!("browser.network.provider"),
            combo(chosen)
                .options(Resolver::ALL.map(Resolver::name))
                .option(tr!("browser.network.custom"))
                .on_select(|shell: &mut Shell, at: usize| shell.choose_resolver(at))
                .width(CHOICE),
        )
    });
    let custom_field = (custom || (mode != DnsMode::Off && resolver.is_none())).then(|| {
        column! {
            row! {
                text_field(&forms.resolver)
                    .placeholder(tr!("browser.network.resolver-placeholder"))
                    .error(forms.problem(Input::Resolver).is_some())
                    .on_submit(|shell: &mut Shell| shell.apply_resolver())
                    .grow(1.0);
                button(text(tr!("browser.network.apply")))
                    .secondary()
                    .small()
                    .on_press(|shell: &mut Shell| shell.apply_resolver());
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width();
            problem_line(forms.problem(Input::Resolver));
        }
        .gap(Space::HAIR)
        .fill_width()
    });
    section(
        title(shell, tr!("browser.network.dns")),
        column! {
            field(
                tr!("browser.network.dns-mode"),
                combo(DnsMode::ALL.iter().position(|candidate| *candidate == mode))
                    .options(DnsMode::ALL.map(dns_mode_name))
                    .on_select(|shell: &mut Shell, at: usize| {
                        if let Some(mode) = DnsMode::ALL.get(at) {
                            shell.choose_dns_mode(*mode);
                        }
                    })
                    .width(CHOICE),
            );
            providers;
            custom_field;
        }
        .gap(Space::NORMAL)
        .fill_width(),
    )
    .any()
}

fn endpoint_field(label: Label, document: &TextDocument, error: Option<InputError>) -> Element {
    column! {
        field(
            label,
            text_field(document)
                .placeholder(tr!("browser.network.endpoint-placeholder"))
                .error(error.is_some())
                .on_submit(|shell: &mut Shell| shell.apply_proxy())
                .width(ENDPOINT),
        );
        problem_line(error);
    }
    .gap(Space::HAIR)
    .fill_width()
    .any()
}

fn apply_row() -> Element {
    row! {
        button(text(tr!("browser.network.apply")))
            .secondary()
            .small()
            .on_press(|shell: &mut Shell| shell.apply_proxy());
    }
    .any()
}

fn proxy(shell: &Shell) -> Element {
    let forms = &shell.sections.web;
    let mode = forms.proxy_mode();
    let details = match mode {
        ProxyMode::Script => Some(
            column! {
                text_field(&forms.script)
                    .placeholder(tr!("browser.network.script-placeholder"))
                    .error(forms.problem(Input::Script).is_some())
                    .on_submit(|shell: &mut Shell| shell.apply_proxy())
                    .fill_width();
                problem_line(forms.problem(Input::Script));
                apply_row();
            }
            .gap(Space::SNUG)
            .fill_width()
            .any(),
        ),
        ProxyMode::Manual => Some(
            column! {
                endpoint_field(tr!("browser.network.http"), &forms.http, forms.problem(Input::Http));
                endpoint_field(tr!("browser.network.https"), &forms.https, forms.problem(Input::Https));
                endpoint_field(tr!("browser.network.socks"), &forms.socks, forms.problem(Input::Socks));
                checkbox(forms.remote_dns())
                    .label(text(tr!("browser.network.remote-dns")))
                    .on_toggle(|shell: &mut Shell, on: bool| shell.sections.web.set_remote_dns(on));
                field(
                    tr!("browser.network.bypass"),
                    text_field(&forms.bypass)
                        .placeholder(tr!("browser.network.bypass-placeholder"))
                        .on_submit(|shell: &mut Shell| shell.apply_proxy())
                        .width(ENDPOINT),
                );
                apply_row();
            }
            .gap(Space::SNUG)
            .fill_width()
            .any(),
        ),
        ProxyMode::System | ProxyMode::Direct | ProxyMode::Detect => None,
    };
    section(
        title(shell, tr!("browser.network.proxy")),
        column! {
            field(
                tr!("browser.network.proxy-mode"),
                combo(ProxyMode::ALL.iter().position(|candidate| *candidate == mode))
                    .options(ProxyMode::ALL.map(proxy_mode_name))
                    .on_select(|shell: &mut Shell, at: usize| {
                        if let Some(mode) = ProxyMode::ALL.get(at) {
                            shell.choose_proxy_mode(*mode);
                        }
                    })
                    .width(CHOICE),
            );
            details;
        }
        .gap(Space::NORMAL)
        .fill_width(),
    )
    .any()
}

pub(crate) fn view(shell: &Shell) -> Element {
    page([dns(shell), proxy(shell)]).any()
}
