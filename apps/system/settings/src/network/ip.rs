use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use rikka_network::{Address, IpMethod, IpProfile, Route};

use crate::picker::choose;

const ADDRESS: f32 = 176.0;
const PREFIX: f32 = 68.0;
const METRIC: f32 = 80.0;
const WIDE: f32 = 300.0;

type Methods = &'static [(IpMethod, &'static str)];

const FOUR: Methods = &[
    (IpMethod::Automatic, "settings.network.ip.automatic"),
    (IpMethod::Manual, "settings.network.ip.manual"),
    (IpMethod::LinkLocal, "settings.network.ip.link-local"),
    (IpMethod::Shared, "settings.network.ip.shared"),
    (IpMethod::Disabled, "settings.network.ip.disabled"),
];

const SIX: Methods = &[
    (IpMethod::Automatic, "settings.network.ip.automatic"),
    (IpMethod::DhcpOnly, "settings.network.ip.dhcp"),
    (IpMethod::Manual, "settings.network.ip.manual"),
    (IpMethod::LinkLocal, "settings.network.ip.link-local"),
    (IpMethod::Shared, "settings.network.ip.shared"),
    (IpMethod::Disabled, "settings.network.ip.disabled"),
];

struct AddressRow {
    address: TextDocument,
    prefix: TextDocument,
}

struct RouteRow {
    destination: TextDocument,
    prefix: TextDocument,
    next_hop: TextDocument,
    metric: TextDocument,
}

#[derive(State)]
pub(crate) struct IpForm {
    six: bool,
    method: IpMethod,
    addresses: Vec<AddressRow>,
    gateway: TextDocument,
    nameservers: TextDocument,
    searches: TextDocument,
    routes: Vec<RouteRow>,
    automatic_nameservers: bool,
    automatic_routes: bool,
    only_its_own_network: bool,
}

impl IpForm {
    pub(crate) fn new(profile: &IpProfile, six: bool) -> Self {
        Self {
            six,
            method: profile.method,
            addresses: profile
                .addresses
                .iter()
                .map(|address| AddressRow {
                    address: TextDocument::new(&address.address),
                    prefix: TextDocument::new(address.prefix.to_string()),
                })
                .collect(),
            gateway: TextDocument::new(&profile.gateway),
            nameservers: TextDocument::new(profile.nameservers.join(", ")),
            searches: TextDocument::new(profile.searches.join(", ")),
            routes: profile
                .routes
                .iter()
                .map(|route| RouteRow {
                    destination: TextDocument::new(&route.destination),
                    prefix: TextDocument::new(route.prefix.to_string()),
                    next_hop: TextDocument::new(&route.next_hop),
                    metric: TextDocument::new(
                        route
                            .metric
                            .map(|metric| metric.to_string())
                            .unwrap_or_default(),
                    ),
                })
                .collect(),
            automatic_nameservers: !profile.ignore_automatic_dns,
            automatic_routes: !profile.ignore_automatic_routes,
            only_its_own_network: profile.never_default,
        }
    }

    pub(crate) fn profile(&self) -> IpProfile {
        IpProfile {
            method: self.method,
            addresses: self
                .addresses
                .iter()
                .filter(|row| !row.address.is_empty())
                .map(|row| Address {
                    address: row.address.text(),
                    prefix: self.prefix_of(&row.prefix),
                })
                .collect(),
            gateway: self.gateway.text(),
            nameservers: list(&self.nameservers),
            searches: list(&self.searches),
            routes: self
                .routes
                .iter()
                .filter(|row| !row.destination.is_empty())
                .map(|row| Route {
                    destination: row.destination.text(),
                    prefix: self.prefix_of(&row.prefix),
                    next_hop: row.next_hop.text(),
                    metric: row.metric.text().parse().ok(),
                })
                .collect(),
            ignore_automatic_dns: !self.automatic_nameservers,
            ignore_automatic_routes: !self.automatic_routes,
            never_default: self.only_its_own_network,
        }
    }

    fn prefix_of(&self, typed: &TextDocument) -> u32 {
        let whole = if self.six { 64 } else { 24 };
        let most = if self.six { 128 } else { 32 };
        typed
            .text()
            .parse()
            .map(|prefix: u32| prefix.min(most))
            .unwrap_or(whole)
    }

    fn default_prefix(&self) -> &'static str {
        if self.six { "64" } else { "24" }
    }

    fn add_address(&mut self) {
        self.addresses.push(AddressRow {
            address: TextDocument::new(""),
            prefix: TextDocument::new(self.default_prefix()),
        });
    }

    fn drop_address(&mut self, row: usize) {
        if row < self.addresses.len() {
            self.addresses.remove(row);
        }
    }

    fn add_route(&mut self) {
        self.routes.push(RouteRow {
            destination: TextDocument::new(""),
            prefix: TextDocument::new(self.default_prefix()),
            next_hop: TextDocument::new(""),
            metric: TextDocument::new(""),
        });
    }

    fn drop_route(&mut self, row: usize) {
        if row < self.routes.len() {
            self.routes.remove(row);
        }
    }

    fn methods(&self) -> Methods {
        if self.six { SIX } else { FOUR }
    }
}

pub(crate) fn view(form: &IpForm) -> impl View {
    column! {
        field(
            tr!("settings.network.ip.method"),
            choose(form.methods(), form.method, |form: &mut IpForm, method: IpMethod| {
                form.method = method;
            }),
        );
        if form.method.is_manual() {
            address_rows(form);
            field(
                tr!("settings.network.ip.gateway"),
                entry(&form.gateway, tr!("settings.network.ip.gateway-hint"), WIDE),
            );
        }
        field(
            tr!("settings.network.ip.automatic-dns"),
            switch(form.automatic_nameservers).on_toggle(|form: &mut IpForm, on: bool| {
                form.automatic_nameservers = on;
            }),
        );
        field(
            tr!("settings.network.ip.nameservers"),
            entry(&form.nameservers, tr!("settings.network.ip.nameservers-hint"), WIDE),
        );
        field(
            tr!("settings.network.ip.searches"),
            entry(&form.searches, tr!("settings.network.ip.searches-hint"), WIDE),
        );
        field(
            tr!("settings.network.ip.automatic-routes"),
            switch(form.automatic_routes).on_toggle(|form: &mut IpForm, on: bool| {
                form.automatic_routes = on;
            }),
        );
        route_rows(form);
        field(
            tr!("settings.network.ip.only-its-own-network"),
            switch(form.only_its_own_network).on_toggle(|form: &mut IpForm, on: bool| {
                form.only_its_own_network = on;
            }),
        );
    }
    .gap(Space::SNUG)
    .fill_width()
}

fn address_rows(form: &IpForm) -> impl View {
    column! {
        heading(tr!("settings.network.ip.addresses"));
        for (index, row) in form.addresses.iter().enumerate() {
            row! {
                entry(&row.address, tr!("settings.network.ip.address-hint"), ADDRESS);
                entry(&row.prefix, tr!("settings.network.ip.prefix-hint"), PREFIX);
                drop_button(move |form: &mut IpForm| form.drop_address(index));
            }
            .gap(Space::SNUG)
            .align_items(Align::Center);
        }
        add_button(
            tr!("settings.network.ip.add-address"),
            |form: &mut IpForm| form.add_address(),
        );
    }
    .gap(Space::SNUG)
}

fn route_rows(form: &IpForm) -> impl View {
    column! {
        heading(tr!("settings.network.ip.routes"));
        for (index, row) in form.routes.iter().enumerate() {
            row! {
                entry(&row.destination, tr!("settings.network.ip.destination-hint"), ADDRESS);
                entry(&row.prefix, tr!("settings.network.ip.prefix-hint"), PREFIX);
                entry(&row.next_hop, tr!("settings.network.ip.next-hop-hint"), ADDRESS);
                entry(&row.metric, tr!("settings.network.ip.metric-hint"), METRIC);
                drop_button(move |form: &mut IpForm| form.drop_route(index));
            }
            .gap(Space::SNUG)
            .align_items(Align::Center);
        }
        add_button(tr!("settings.network.ip.add-route"), |form: &mut IpForm| form.add_route());
    }
    .gap(Space::SNUG)
}

fn heading(label: Label) -> impl View {
    text(label).label().muted()
}

fn entry(document: &TextDocument, hint: Label, width: f32) -> impl View {
    text_field(document)
        .placeholder(hint)
        .control_size(ControlSize::Small)
        .on_change(|| {})
        .width(width)
}

fn drop_button<M>(press: impl IntoHandler<(), (), M>) -> impl View {
    button(icon(MaterialSymbol::REMOVE))
        .ghost()
        .small()
        .icon_only()
        .on_press(press)
}

fn add_button<M>(label: Label, press: impl IntoHandler<(), (), M>) -> impl View {
    button(
        row! {
            icon(MaterialSymbol::ADD).extent(Length::px(16.0));
            text(label).label();
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center),
    )
    .ghost()
    .small()
    .on_press(press)
}

fn list(document: &TextDocument) -> Vec<String> {
    document
        .text()
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}
