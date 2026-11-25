use launcher_auth::{Account, DeviceCode, Session};
use rikka_app::prelude::*;
use rikka_ui::Element;
use rikka_widgets::{column, row};

use crate::launcher::{Confirm, Launcher, STAGE};
use crate::skins;

const SIGN_IN: &str = "launcher-sign-in";
const SHEET: f32 = 440.0;
const AVATAR: f32 = 32.0;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Prompt {
    user_code: String,
    url: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SignIn {
    code: Option<Prompt>,
    error: Option<String>,
}

impl Launcher {
    pub(crate) fn begin_sign_in(&mut self) {
        let Some(authenticator) = self.authenticator.clone() else {
            self.fail(tr!("launcher.accounts.keyring-wait"));
            return;
        };
        self.account_menu = false;
        self.signing = Some(SignIn::default());
        let started = authenticator.clone();
        spawn_keyed(
            SIGN_IN,
            async move { started.begin().await },
            move |launcher: &mut Launcher, begun: launcher_auth::Result<DeviceCode>| {
                let Some(signing) = &mut launcher.signing else {
                    return;
                };
                match begun {
                    Ok(code) => {
                        signing.code = Some(Prompt {
                            user_code: code.user_code().to_owned(),
                            url: code.verification_url().to_owned(),
                        });
                        let authenticator = authenticator.clone();
                        spawn_keyed(
                            SIGN_IN,
                            async move {
                                let session = authenticator.complete(code).await?;
                                launcher_auth::keep(&session).await?;
                                Ok(session)
                            },
                            Launcher::signed_in,
                        );
                    }
                    Err(error) => signing.error = Some(error.to_string()),
                }
            },
        );
    }

    fn signed_in(&mut self, session: launcher_auth::Result<Session>) {
        match session {
            Ok(session) => {
                self.signing = None;
                self.accounts.remember(&session);
                self.save_accounts();
                self.tell(tr!(
                    "launcher.accounts.signed-in",
                    name = session.name.as_str()
                ));
                self.load_skins();
            }
            Err(error) => {
                if let Some(signing) = &mut self.signing {
                    signing.error = Some(error.to_string());
                    signing.code = None;
                }
            }
        }
    }

    fn cancel_sign_in(&mut self) {
        cancel(SIGN_IN);
        self.signing = None;
    }

    fn open_verification(&mut self) {
        let Some(url) = self
            .signing
            .as_ref()
            .and_then(|signing| signing.code.as_ref())
            .map(|prompt| prompt.url.clone())
        else {
            return;
        };
        if let Err(error) = rikka_open::open(&url) {
            self.fail(error.to_string());
        }
    }

    pub(crate) fn save_accounts(&mut self) {
        let Some(services) = self.services() else {
            return;
        };
        if let Err(error) = self.accounts.save(&services.accounts_file()) {
            self.fail(error.to_string());
        }
    }

    fn select_account(&mut self, id: String) {
        self.accounts.select(&id);
        self.account_menu = false;
        self.save_accounts();
        self.skins = skins::Skins::default();
        self.load_skins();
    }

    fn toggle_account_menu(&mut self) {
        self.account_menu = !self.account_menu;
    }

    pub(crate) fn remove_account(&mut self, id: &str) {
        self.accounts.forget(id);
        self.save_accounts();
        self.skins = skins::Skins::default();
        self.load_skins();
        let id = id.to_owned();
        spawn(
            async move { launcher_auth::forget(&id).await },
            |launcher: &mut Launcher, forgotten: launcher_auth::Result<()>| {
                if let Err(error) = forgotten {
                    launcher.fail(error.to_string());
                }
            },
        );
    }
}

fn initial_avatar(name: &str, side: f32) -> Element {
    let initial: String = name
        .chars()
        .next()
        .map(|first| first.to_uppercase().collect())
        .unwrap_or_default();
    container(text(initial).label().bold().color(palette().on_accent))
        .center()
        .background(palette().accent)
        .radius(side / 2.0)
        .size(Length::px(side), Length::px(side))
        .any()
}

fn avatar(launcher: &Launcher, account: &Account, side: f32) -> Element {
    match skins::face(launcher, &account.id) {
        Some(face) => container(image(face).fit(ImageFit::Cover).radius(6.0).fill())
            .size(Length::px(side), Length::px(side))
            .any(),
        None => initial_avatar(&account.name, side),
    }
}

pub(crate) fn sidebar_card(launcher: &Launcher) -> Element {
    let active = launcher.accounts.active().cloned();
    let card = match &active {
        Some(account) => container(
            row! {
                avatar(launcher, account, AVATAR);
                column! {
                    text(account.name.clone()).label().semibold().no_wrap().fill_width();
                    text(tr!("launcher.accounts.microsoft")).caption().muted().no_wrap();
                }
                .gap(Space::HAIR)
                .grow(1.0)
                .min_width(0.0);
                icon(match launcher.account_menu {
                    true => MaterialSymbol::EXPAND_MORE,
                    false => MaterialSymbol::EXPAND_LESS,
                })
                .extent(Length::px(18.0))
                .color(palette().text_muted);
            }
            .gap(Space::SNUG)
            .align_items(Align::Center),
        )
        .padding(Inset::all(Space::SNUG))
        .radius(Corner::Panel)
        .background(palette().surface_variant)
        .on_press(Launcher::toggle_account_menu)
        .fill_width()
        .any(),
        None => button(crate::widgets::labelled(
            MaterialSymbol::LOGIN,
            tr!("launcher.accounts.sign-in"),
        ))
        .primary()
        .on_press(Launcher::begin_sign_in)
        .fill_width()
        .any(),
    };
    let menu = launcher.account_menu.then(|| {
        let mut entries: Vec<Element> = launcher
            .accounts
            .list()
            .iter()
            .map(|account| account_line(launcher, account, active.as_ref()))
            .collect();
        entries.push(
            button(crate::widgets::labelled(
                MaterialSymbol::PERSON_ADD,
                tr!("launcher.accounts.add"),
            ))
            .ghost()
            .on_press(Launcher::begin_sign_in)
            .fill_width()
            .any(),
        );
        column_of(entries)
            .gap(Space::HAIR)
            .fill_width()
            .transition(fade())
            .any()
    });
    column! {
        menu;
        card;
    }
    .gap(Space::SNUG)
    .fill_width()
    .any()
}

fn account_line(launcher: &Launcher, account: &Account, active: Option<&Account>) -> Element {
    let chosen = active.is_some_and(|active| active.id == account.id);
    let selected = account.id.clone();
    let removed = (account.id.clone(), account.name.clone());
    row! {
        container(
            row! {
                avatar(launcher, account, 24.0);
                text(account.name.clone()).label().no_wrap().grow(1.0).min_width(0.0);
                if chosen {
                    icon(MaterialSymbol::CHECK).extent(Length::px(16.0)).color(palette().accent);
                }
            }
            .gap(Space::SNUG)
            .align_items(Align::Center),
        )
        .padding(Inset::xy(Space::SNUG, Space::TIGHT))
        .radius(Corner::Item)
        .on_press(move |launcher: &mut Launcher| launcher.select_account(selected.clone()))
        .grow(1.0)
        .min_width(0.0);
        crate::widgets::icon_button(
            MaterialSymbol::LOGOUT,
            tr!("launcher.accounts.remove"),
            move |launcher: &mut Launcher| {
                launcher.ask(Confirm::RemoveAccount {
                    id: removed.0.clone(),
                    name: removed.1.clone(),
                });
            },
        );
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .fill_width()
    .any()
}

pub(crate) fn sign_in_sheet(launcher: &Launcher) -> Vec<Element> {
    let Some(signing) = &launcher.signing else {
        return Vec::new();
    };
    let body = match (&signing.code, &signing.error) {
        (_, Some(error)) => column! {
            text(error.clone()).label().danger().fill_width();
            button(text(tr!("launcher.accounts.retry")))
                .secondary()
                .on_press(Launcher::begin_sign_in);
        }
        .gap(Space::NORMAL)
        .fill_width()
        .any(),
        (None, None) => row! {
            spinner().extent(Length::px(18.0));
            text(tr!("launcher.accounts.contacting")).label().muted();
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .any(),
        (
            Some(Prompt {
                user_code: code,
                url,
            }),
            None,
        ) => column! {
            text(tr!("launcher.accounts.step-open", url = url.as_str())).label().fill_width();
            button(crate::widgets::labelled(
                MaterialSymbol::OPEN_IN_NEW,
                tr!("launcher.accounts.open-link"),
            ))
            .primary()
            .on_press(Launcher::open_verification);
            text(tr!("launcher.accounts.step-code")).label().fill_width();
            container(
                row! {
                    text(code.clone())
                        .title()
                        .bold()
                        .family(Family::Monospace)
                        .grow(1.0);
                    copy_button(code.as_str());
                }
                .align_items(Align::Center),
            )
            .padding(Inset::xy(Space::ROOMY, Space::NORMAL))
            .radius(Corner::Control)
            .background(palette().surface_variant)
            .fill_width();
            row! {
                spinner().extent(Length::px(16.0));
                text(tr!("launcher.accounts.waiting")).caption().muted();
            }
            .gap(Space::SNUG)
            .align_items(Align::Center);
        }
        .gap(Space::NORMAL)
        .fill_width()
        .any(),
    };
    sheet(
        column! {
            row! {
                icon(MaterialSymbol::ACCOUNT_CIRCLE).extent(Length::px(24.0)).color(palette().accent);
                text(tr!("launcher.accounts.sign-in-title")).label().semibold();
            }
            .gap(Space::SNUG)
            .align_items(Align::Center);
            text(tr!("launcher.accounts.sign-in-body")).caption().muted().fill_width();
            body;
            row! {
                spacer().grow(1.0);
                button(text(tr!("launcher.action.cancel")))
                    .ghost()
                    .on_press(Launcher::cancel_sign_in);
            }
            .fill_width();
        }
        .gap(Space::ROOMY)
        .padding(20.0)
        .fill_width(),
    )
    .width(SHEET)
    .on_dismiss(Launcher::cancel_sign_in)
    .keyed("launcher-sign-in")
    .centered_on(STAGE)
    .with_scrim()
}
