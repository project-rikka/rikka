use rikka_app::prelude::*;
use rikka_barcode::QrCode;
use rikka_widgets::column;

use rikka_network::{
    Device, Invitation, MacAddress, Metered, Profile, ProxyMethod, Secrets, Security, Snapshot,
};

use crate::info_row::info_row;
use crate::network::ip::{self, IpForm};
use crate::network::{format, link};
use crate::picker::{choose, picker};
use crate::typed::secret;

const WIDE: f32 = 300.0;
const NARROW: f32 = 120.0;
const CODE: f32 = 200.0;
const CODE_MODULE: u32 = 6;
const REVEAL: f32 = 40.0;

const METERED: [(Metered, &str); 3] = [
    (Metered::Automatic, "settings.network.metered.automatic"),
    (Metered::Yes, "settings.network.metered.yes"),
    (Metered::No, "settings.network.metered.no"),
];

const ADDRESSES: [(MacAddress, &str); 5] = [
    (MacAddress::Unset, "settings.network.mac.default"),
    (MacAddress::Permanent, "settings.network.mac.permanent"),
    (MacAddress::Preserve, "settings.network.mac.preserve"),
    (MacAddress::Random, "settings.network.mac.random"),
    (MacAddress::Stable, "settings.network.mac.stable"),
];

const PROXIES: [(ProxyMethod, &str); 2] = [
    (ProxyMethod::None, "settings.network.proxy.none"),
    (ProxyMethod::Script, "settings.network.proxy.script"),
];

#[derive(State)]
pub(crate) struct Detail {
    pub(crate) uuid: String,
    saved: Profile,
    draft: Profile,
    name: TextDocument,
    priority: TextDocument,
    mtu: TextDocument,
    fixed: TextDocument,
    passphrase: TextDocument,
    script: TextDocument,
    revealed: bool,
    shown: bool,
    code: Option<ImageSource>,
    ipv4: IpForm,
    ipv6: IpForm,
}

impl Detail {
    pub(crate) fn new(profile: &Profile) -> Self {
        Self {
            uuid: profile.uuid.clone(),
            saved: profile.clone(),
            name: TextDocument::new(&profile.id),
            priority: TextDocument::new(profile.autoconnect_priority.to_string()),
            mtu: TextDocument::new(if profile.mtu == 0 {
                String::new()
            } else {
                profile.mtu.to_string()
            }),
            fixed: TextDocument::new(match &profile.address {
                MacAddress::Fixed(address) => address.as_str(),
                _ => "",
            }),
            passphrase: TextDocument::new(""),
            script: TextDocument::new(&profile.proxy.script_url),
            revealed: false,
            shown: false,
            code: None,
            ipv4: IpForm::new(&profile.ipv4, false),
            ipv6: IpForm::new(&profile.ipv6, true),
            draft: profile.clone(),
        }
    }

    pub(crate) fn reveal(&mut self, secrets: &Secrets) {
        self.revealed = true;
        self.shown = true;
        if let Some(passphrase) = &secrets.passphrase {
            self.passphrase.set_text(passphrase.as_str());
        }
        if let Some(security) = &mut self.saved.security {
            security.passphrase = secrets.passphrase.clone();
        }
        self.code = self.share_code();
    }

    fn share_code(&self) -> Option<ImageSource> {
        let wireless = self.draft.wireless.as_ref()?;
        let security = self
            .draft
            .security
            .as_ref()
            .map(|security| security.security)
            .unwrap_or(Security::Open);
        let invitation = Invitation {
            ssid: wireless.ssid.clone(),
            security,
            passphrase: self.passphrase.text(),
            hidden: wireless.hidden,
        };
        let code = QrCode::encode(&invitation.to_string()).ok()?;
        let image = code.rasterize(CODE_MODULE);
        Some(ImageSource::Rgba {
            width: image.side(),
            height: image.side(),
            pixels: image.into_rgba().into(),
        })
    }

    pub(crate) fn is_edited(&self) -> bool {
        self.profile() != self.saved
    }

    pub(crate) fn profile(&self) -> Profile {
        let mut profile = self.draft.clone();
        profile.id = self.name.text();
        profile.autoconnect_priority = self.priority.text().parse().unwrap_or(0);
        profile.mtu = self.mtu.text().parse().unwrap_or(0);
        profile.ipv4 = self.ipv4.profile();
        profile.ipv6 = self.ipv6.profile();
        profile.proxy.script_url = self.script.text();
        if let MacAddress::Fixed(_) = profile.address {
            profile.address = MacAddress::Fixed(self.fixed.text());
        }
        if let Some(security) = &mut profile.security {
            let typed = self.passphrase.text();
            security.passphrase = (!typed.is_empty()).then_some(typed);
        }
        profile
    }

    pub(crate) fn title(&self) -> String {
        self.name.text()
    }

    pub(crate) fn has_secret(&self) -> bool {
        self.saved.has_secret() && !self.revealed
    }

    fn address_index(&self) -> usize {
        ADDRESSES
            .iter()
            .position(|(address, _)| *address == self.draft.address)
            .unwrap_or(ADDRESSES.len())
    }
}

pub(crate) fn view(detail: &Detail, snapshot: &Snapshot) -> impl View {
    let device = snapshot
        .devices
        .iter()
        .find(|device| device.active.as_deref() == Some(detail.uuid.as_str()));
    column! {
        connection_details(device, snapshot);
        identity(detail);
        share(detail);
        section(
            tr!("settings.network.detail.ipv4"),
            scope(|detail: &mut Detail| &mut detail.ipv4, ip::view(&detail.ipv4)),
        );
        section(
            tr!("settings.network.detail.ipv6"),
            scope(|detail: &mut Detail| &mut detail.ipv6, ip::view(&detail.ipv6)),
        );
        proxy(detail);
    }
    .gap(Space::ROOMY)
    .fill_width()
}

fn connection_details(device: Option<&Device>, snapshot: &Snapshot) -> Option<impl View> {
    let device = device?;
    let mut rows: Vec<Field> = Vec::new();
    let point = device
        .wireless
        .as_ref()
        .and_then(|wireless| wireless.access_point.as_ref())
        .and_then(|point| {
            snapshot
                .access_points
                .iter()
                .find(|access| &access.id == point)
        });
    if let Some(point) = point {
        rows.extend(info_row(
            tr!("settings.network.detail.strength"),
            format!("{}%", point.strength),
        ));
        rows.extend(info_row(
            tr!("settings.network.detail.security"),
            format::security(point.security).to_string(),
        ));
        let band = format::band(point.band());
        let channel = point.channel().to_string();
        rows.extend(info_row(
            tr!("settings.network.detail.band"),
            tr!(
                "settings.network.detail.channel",
                band = band.as_str(),
                channel = &channel
            )
            .to_string(),
        ));
        rows.extend(info_row(
            tr!("settings.network.detail.bssid"),
            point.bssid.clone(),
        ));
    }
    rows.extend(link::link_details(device));
    if rows.is_empty() {
        return None;
    }
    Some(section(
        tr!("settings.network.detail.facts"),
        column_of(rows).gap(Space::SNUG).fill_width(),
    ))
}

fn identity(detail: &Detail) -> impl View {
    section(
        tr!("settings.network.detail.identity"),
        column! {
            field(
                tr!("settings.network.detail.name"),
                text_field(&detail.name).on_change(|| {}).width(WIDE),
            );
            field(
                tr!("settings.network.detail.autoconnect"),
                switch(detail.draft.autoconnect)
                    .on_toggle(|detail: &mut Detail, on: bool| detail.draft.autoconnect = on),
            );
            field(
                tr!("settings.network.detail.priority"),
                text_field(&detail.priority)
                    .placeholder(tr!("settings.network.detail.priority-hint"))
                    .on_change(|| {})
                    .width(NARROW),
            );
            field(
                tr!("settings.network.detail.shared"),
                switch(detail.draft.shared)
                    .on_toggle(|detail: &mut Detail, on: bool| detail.draft.shared = on),
            );
            field(
                tr!("settings.network.detail.metered"),
                choose(
                    &METERED,
                    detail.draft.metered,
                    |detail: &mut Detail, metered: Metered| detail.draft.metered = metered,
                ),
            );
            if detail.draft.kind.is_physical() {
                field(
                    tr!("settings.network.detail.mtu"),
                    text_field(&detail.mtu)
                        .placeholder(tr!("settings.network.detail.mtu-hint"))
                        .on_change(|| {})
                        .width(NARROW),
                );
                field(
                    tr!("settings.network.detail.address"),
                    picker(
                        Some(detail.address_index()),
                        ADDRESSES
                            .iter()
                            .map(|(_, key)| Label::translated(key))
                            .chain(std::iter::once(tr!("settings.network.mac.fixed"))),
                        |detail: &mut Detail, index: usize| {
                            detail.draft.address = match ADDRESSES.get(index) {
                                Some((address, _)) => address.clone(),
                                None => MacAddress::Fixed(detail.fixed.text()),
                            };
                        },
                    ),
                );
                if matches!(detail.draft.address, MacAddress::Fixed(_)) {
                    field(
                        tr!("settings.network.mac.fixed"),
                        text_field(&detail.fixed)
                            .placeholder(tr!("settings.network.mac.fixed-hint"))
                            .on_change(|detail: &mut Detail| {
                                detail.draft.address = MacAddress::Fixed(detail.fixed.text());
                            })
                            .width(WIDE),
                    );
                }
            }
        }
        .gap(Space::SNUG)
        .fill_width(),
    )
}

fn share(detail: &Detail) -> Option<impl View> {
    if detail.draft.wireless.is_none() || !detail.saved.has_secret() {
        return None;
    }
    Some(section(
        tr!("settings.network.detail.password"),
        column! {
            field(
                tr!("settings.network.detail.passphrase"),
                secret(
                    &detail.passphrase,
                    tr!("settings.network.detail.passphrase-hidden"),
                    detail.shown,
                    WIDE - REVEAL,
                    |detail: &mut Detail| detail.code = detail.share_code(),
                    |detail: &mut Detail, shown: bool| detail.shown = shown,
                ),
            );
            detail.code.as_ref().map(|code| {
                container(
                    image(code.clone())
                        .fit(ImageFit::Contain)
                        .width(CODE)
                        .height(CODE),
                )
                .radius(Corner::Item)
                .padding(Space::SNUG)
                .background(Color::rgb8(255, 255, 255))
            });
        }
        .gap(Space::SNUG)
        .fill_width(),
    ))
}

fn proxy(detail: &Detail) -> impl View {
    section(
        tr!("settings.network.detail.proxy"),
        column! {
            field(
                tr!("settings.network.proxy.method"),
                choose(
                    &PROXIES,
                    detail.draft.proxy.method,
                    |detail: &mut Detail, method: ProxyMethod| detail.draft.proxy.method = method,
                ),
            );
            if detail.draft.proxy.method == ProxyMethod::Script {
                field(
                    tr!("settings.network.proxy.script-url"),
                    text_field(&detail.script)
                        .placeholder(tr!("settings.network.proxy.script-hint"))
                        .on_change(|| {})
                        .width(WIDE),
                );
                field(
                    tr!("settings.network.proxy.browser-only"),
                    switch(detail.draft.proxy.browser_only).on_toggle(
                        |detail: &mut Detail, on: bool| detail.draft.proxy.browser_only = on,
                    ),
                );
            }
        }
        .gap(Space::SNUG)
        .fill_width(),
    )
}
