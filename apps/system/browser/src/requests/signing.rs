use rikka_app::prelude::*;
use rikka_web::{AuthFlags, SignIn};
use rikka_widgets::{column, row};

use crate::requests::sheets::framed;
use crate::shell::Shell;
use crate::widgets::confirm_keys;
use browser_model::TabId;

#[derive(Debug)]
pub(crate) struct Signing {
    pub(crate) request: SignIn,
    user: TextDocument,
    password: TextDocument,
}

impl Signing {
    fn new(request: SignIn) -> Signing {
        let user = TextDocument::new(request.username.clone().unwrap_or_default());
        Signing {
            request,
            user,
            password: TextDocument::new(""),
        }
    }

    fn asks_user(&self) -> bool {
        !self.request.flags.contains(AuthFlags::ONLY_PASSWORD)
    }
}

impl Shell {
    pub(crate) fn sign_in_asked(&mut self, tab: TabId, request: SignIn) {
        let replaced = self
            .browser()
            .page_services()
            .requests_mut()
            .edit(tab)
            .signing
            .replace(Signing::new(request));
        drop(replaced);
    }

    pub(crate) fn answer_sign_in(&mut self, tab: TabId, answered: bool) {
        let Some(signing) = self
            .browser()
            .page_services()
            .requests_mut()
            .edit_existing(tab)
            .and_then(|requests| requests.signing.take())
        else {
            return;
        };
        if !answered {
            signing.request.dismiss();
            return;
        }
        let user = signing.asks_user().then(|| signing.user.text());
        let password = signing.password.text();
        signing.request.submit(user, password);
    }
}

fn place(request: &SignIn) -> String {
    request
        .uri
        .as_deref()
        .and_then(|uri| url::Url::parse(uri).ok())
        .and_then(|parsed| {
            let host = parsed.host_str()?.to_owned();
            Some(match parsed.port() {
                Some(port) => format!("{host}:{port}"),
                None => host,
            })
        })
        .unwrap_or_default()
}

pub(crate) fn card(signing: &Signing, tab: TabId) -> Element {
    let request = &signing.request;
    let title = match request.flags.contains(AuthFlags::PROXY) {
        true => tr!("browser.sign-in.proxy-title"),
        false => tr!("browser.sign-in.title"),
    };
    let message = request
        .message
        .clone()
        .filter(|message| !message.is_empty());
    let failed = request.flags.contains(AuthFlags::PREVIOUS_FAILED);
    let user = signing.asks_user().then(|| {
        text_field(&signing.user)
            .placeholder(tr!("browser.sign-in.user"))
            .claims_focus(true)
            .on_submit(move |shell: &mut Shell| shell.answer_sign_in(tab, true))
            .fill_width()
    });
    let card = framed(
        column! {
            text(title).label().semibold();
            text(place(request)).caption().muted();
            message.map(|message| text(message).caption().muted());
            failed.then(|| {
                text(tr!("browser.sign-in.failed"))
                    .caption()
                    .color(palette().danger)
            });
            user;
            text_field(&signing.password)
                .placeholder(tr!("browser.sign-in.password"))
                .secret(true)
                .claims_focus(!signing.asks_user())
                .on_submit(move |shell: &mut Shell| shell.answer_sign_in(tab, true))
                .fill_width();
            row! {
                spacer().grow(1.0);
                button(text(tr!("browser.dialog.cancel")))
                    .secondary()
                    .on_press(move |shell: &mut Shell| shell.answer_sign_in(tab, false));
                button(text(tr!("browser.sign-in.sign-in")))
                    .primary()
                    .on_press(move |shell: &mut Shell| shell.answer_sign_in(tab, true));
            }
            .gap(Space::SNUG)
            .fill_width();
        }
        .gap(Space::NORMAL)
        .fill_width(),
    );
    confirm_keys(
        card,
        move |shell: &mut Shell| shell.answer_sign_in(tab, true),
        move |shell: &mut Shell| shell.answer_sign_in(tab, false),
    )
}
