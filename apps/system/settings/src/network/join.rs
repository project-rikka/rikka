use rikka_app::prelude::*;
use rikka_widgets::column;

use rikka_network::{DeviceId, Eap, EapMethod, Join, JoinTarget, Phase2, Security, Ssid};

use crate::picker::{choose, picker};
use crate::typed::secret;

const WIDE: f32 = 300.0;
const REVEAL: f32 = 40.0;

const SECURITIES: [(Security, &str); 5] = [
    (Security::Open, "settings.network.security.open"),
    (Security::Personal, "settings.network.security.personal"),
    (Security::Wpa3Personal, "settings.network.security.wpa3"),
    (Security::Wep, "settings.network.security.wep"),
    (Security::Enterprise, "settings.network.security.enterprise"),
];

const METHODS: [(EapMethod, &str); 4] = [
    (EapMethod::Peap, "settings.network.eap.peap"),
    (EapMethod::Ttls, "settings.network.eap.ttls"),
    (EapMethod::Tls, "settings.network.eap.tls"),
    (EapMethod::Pwd, "settings.network.eap.pwd"),
];

const PHASE2: [(Phase2, &str); 6] = [
    (Phase2::Mschapv2, "settings.network.phase2.mschapv2"),
    (Phase2::Pap, "settings.network.phase2.pap"),
    (Phase2::Chap, "settings.network.phase2.chap"),
    (Phase2::Mschap, "settings.network.phase2.mschap"),
    (Phase2::Md5, "settings.network.phase2.md5"),
    (Phase2::Gtc, "settings.network.phase2.gtc"),
];

enum Target {
    Visible {
        point: rikka_network::ApId,
        ssid: Ssid,
    },
    Hidden,
}

#[derive(State)]
pub(crate) struct JoinForm {
    device: DeviceId,
    target: Target,
    security: Security,
    name: TextDocument,
    passphrase: TextDocument,
    show: bool,
    shared: bool,
    method: EapMethod,
    identity: TextDocument,
    anonymous: TextDocument,
    domain: TextDocument,
    authority: TextDocument,
    certificate: TextDocument,
    private_key: TextDocument,
    key_password: TextDocument,
    phase2: Phase2,
    password: TextDocument,
}

impl JoinForm {
    pub(crate) fn visible(device: DeviceId, point: &rikka_network::AccessPoint) -> Self {
        Self::new(
            device,
            Target::Visible {
                point: point.id.clone(),
                ssid: point.ssid.clone(),
            },
            point.security,
        )
    }

    pub(crate) fn hidden(device: DeviceId) -> Self {
        Self::new(device, Target::Hidden, Security::Personal)
    }

    fn new(device: DeviceId, target: Target, security: Security) -> Self {
        Self {
            device,
            target,
            security,
            name: TextDocument::new(""),
            passphrase: TextDocument::new(""),
            show: false,
            shared: false,
            method: EapMethod::default(),
            identity: TextDocument::new(""),
            anonymous: TextDocument::new(""),
            domain: TextDocument::new(""),
            authority: TextDocument::new(""),
            certificate: TextDocument::new(""),
            private_key: TextDocument::new(""),
            key_password: TextDocument::new(""),
            phase2: Phase2::default(),
            password: TextDocument::new(""),
        }
    }

    pub(crate) fn title(&self) -> String {
        match &self.target {
            Target::Visible { ssid, .. } => ssid.name().into_owned(),
            Target::Hidden => self.name.text(),
        }
    }

    fn is_hidden(&self) -> bool {
        matches!(self.target, Target::Hidden)
    }

    pub(crate) fn is_ready(&self) -> bool {
        if self.is_hidden() && self.name.is_empty() {
            return false;
        }
        if self.security.needs_account() {
            return self.account().is_complete();
        }
        !self.security.needs_passphrase() || !self.passphrase.is_empty()
    }

    pub(crate) fn request(&self) -> Join {
        Join {
            device: self.device.clone(),
            network: match &self.target {
                Target::Visible { point, .. } => JoinTarget::Visible(point.clone()),
                Target::Hidden => JoinTarget::Hidden(Ssid::from(self.name.text().as_str())),
            },
            security: self.security,
            passphrase: self.passphrase.text(),
            account: self
                .security
                .needs_account()
                .then(|| Box::new(self.account())),
            shared: self.shared,
        }
    }

    fn account(&self) -> Eap {
        Eap {
            method: self.method,
            identity: self.identity.text(),
            anonymous_identity: self.anonymous.text(),
            authority: path(&self.authority),
            domain_suffix: self.domain.text(),
            certificate: path(&self.certificate),
            private_key: path(&self.private_key),
            private_key_password: non_empty_text(&self.key_password),
            phase2: self.phase2,
            password: non_empty_text(&self.password),
        }
    }
}

pub(crate) fn view(form: &JoinForm) -> impl View {
    column! {
        if form.is_hidden() {
            field(
                tr!("settings.network.join.name"),
                entry(&form.name, tr!("settings.network.join.name-hint")),
            );
            field(
                tr!("settings.network.join.security"),
                choose(&SECURITIES, form.security, |form: &mut JoinForm, security: Security| {
                    form.security = security;
                }),
            );
        }
        if form.security.needs_passphrase() {
            field(
                tr!("settings.network.join.passphrase"),
                hidden(
                    form,
                    &form.passphrase,
                    tr!("settings.network.join.passphrase-hint"),
                ),
            );
        }
        if form.security.needs_account() {
            account_rows(form);
        }
        field(
            tr!("settings.network.join.shared"),
            switch(form.shared).on_toggle(|form: &mut JoinForm, shared: bool| form.shared = shared),
        );
    }
    .gap(Space::SNUG)
    .fill_width()
}

fn hidden(form: &JoinForm, document: &TextDocument, hint: Label) -> impl View {
    secret(
        document,
        hint,
        form.show,
        WIDE - REVEAL,
        || {},
        |form: &mut JoinForm, show: bool| form.show = show,
    )
}

fn account_rows(form: &JoinForm) -> impl View {
    column! {
        field(
            tr!("settings.network.eap.method"),
            choose(&METHODS, form.method, |form: &mut JoinForm, method: EapMethod| {
                form.method = method;
            }),
        );
        if form.method.is_certificate() {
            field(
                tr!("settings.network.eap.certificate"),
                entry(&form.certificate, tr!("settings.network.eap.file-hint")),
            );
            field(
                tr!("settings.network.eap.private-key"),
                entry(&form.private_key, tr!("settings.network.eap.file-hint")),
            );
            field(
                tr!("settings.network.eap.key-password"),
                hidden(form, &form.key_password, tr!("settings.network.eap.key-password")),
            );
        }
        field(
            tr!("settings.network.eap.identity"),
            entry(&form.identity, tr!("settings.network.eap.identity-hint")),
        );
        if !form.method.is_certificate() {
            field(
                tr!("settings.network.eap.password"),
                hidden(form, &form.password, tr!("settings.network.eap.password")),
            );
        }
        if form.method.is_tunnelled() {
            field(
                tr!("settings.network.eap.phase2"),
                picker(
                    form.method
                        .inner()
                        .iter()
                        .position(|phase2| *phase2 == form.phase2),
                    form.method
                        .inner()
                        .iter()
                        .map(|phase2| Label::translated(inner_label(*phase2))),
                    |form: &mut JoinForm, index: usize| {
                        if let Some(phase2) = form.method.inner().get(index) {
                            form.phase2 = *phase2;
                        }
                    },
                ),
            );
            field(
                tr!("settings.network.eap.anonymous"),
                entry(&form.anonymous, tr!("settings.network.eap.anonymous-hint")),
            );
        }
        field(
            tr!("settings.network.eap.authority"),
            entry(&form.authority, tr!("settings.network.eap.file-hint")),
        );
        field(
            tr!("settings.network.eap.domain"),
            entry(&form.domain, tr!("settings.network.eap.domain-hint")),
        );
    }
    .gap(Space::SNUG)
    .fill_width()
}

fn inner_label(phase2: Phase2) -> &'static str {
    PHASE2
        .iter()
        .find(|(method, _)| *method == phase2)
        .map(|(_, label)| *label)
        .unwrap_or("settings.network.phase2.mschapv2")
}

fn entry(document: &TextDocument, hint: Label) -> impl View {
    text_field(document)
        .placeholder(hint)
        .on_change(|| {})
        .width(WIDE)
}

fn path(document: &TextDocument) -> Option<std::path::PathBuf> {
    let text = document.text();
    (!text.is_empty()).then(|| std::path::PathBuf::from(text))
}

fn non_empty_text(document: &TextDocument) -> Option<String> {
    let text = document.text();
    (!text.is_empty()).then_some(text)
}
