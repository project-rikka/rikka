use proton_session::{Failure, MailboxKey, SecondFactor};
use rikka_app::prelude::*;
use rikka_widgets::column;

use crate::desk::Desk;
use crate::pane;

const CARD: f32 = 380.0;

#[derive(Debug, Default)]
pub(crate) enum Stage {
    #[default]
    Opening,
    Fresh,
    Locked(String),
    Code(Box<SecondFactor>),
    Mailbox(Box<MailboxKey>),
}

#[derive(Debug, State)]
pub(crate) struct Signing {
    pub(crate) stage: Stage,
    pub(crate) username: TextDocument,
    pub(crate) password: TextDocument,
    pub(crate) code: TextDocument,
    pub(crate) mailbox: TextDocument,
    pub(crate) busy: bool,
    error: Option<Failure>,
}

impl Signing {
    pub(crate) fn new() -> Signing {
        Signing {
            stage: Stage::Opening,
            username: TextDocument::new(""),
            password: TextDocument::new(""),
            code: TextDocument::new(""),
            mailbox: TextDocument::new(""),
            busy: false,
            error: None,
        }
    }

    pub(crate) fn take(&mut self) -> Stage {
        std::mem::replace(&mut self.stage, Stage::Fresh)
    }

    pub(crate) fn working(&mut self) {
        self.busy = true;
        self.error = None;
    }

    pub(crate) fn fresh(&mut self) {
        self.busy = false;
        self.stage = Stage::Fresh;
    }

    pub(crate) fn lock(&mut self, username: String) {
        self.busy = false;
        self.username.set_text(&username);
        self.stage = Stage::Locked(username);
    }

    pub(crate) fn ask_for_code(&mut self, pending: Box<SecondFactor>) {
        self.busy = false;
        self.code.clear();
        self.stage = Stage::Code(pending);
    }

    pub(crate) fn ask_for_mailbox(&mut self, pending: Box<MailboxKey>) {
        self.busy = false;
        self.mailbox.clear();
        self.stage = Stage::Mailbox(pending);
    }

    pub(crate) fn failed(&mut self, failure: Failure) {
        self.busy = false;
        self.error = Some(failure);
    }

    pub(crate) fn settled(&mut self) {
        self.busy = false;
        self.error = None;
        self.password.clear();
        self.code.clear();
        self.mailbox.clear();
    }
}

pub(crate) fn view(signing: &Signing) -> Element {
    container(
        column! {
            icon(MaterialSymbol::SHIELD)
                .extent(Length::px(38.0))
                .color(palette().accent);
            text(tr!("proton.signing.title")).title().semibold();
            container(body(signing)).width(CARD);
            if let Some(error) = &signing.error {
                container(
                    text(error.message().to_owned())
                        .caption()
                        .color(palette().danger)
                        .align(TextAlign::Center)
                        .max_width(CARD),
                );
            }
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center),
    )
    .center()
    .fill()
    .build()
}

fn body(signing: &Signing) -> Element {
    if matches!(signing.stage, Stage::Opening) {
        return pane::working(tr!("proton.signing.opening"))
            .height(120.0)
            .build();
    }

    match &signing.stage {
        Stage::Locked(username) => locked(signing, username).build(),
        Stage::Code(pending) => code(signing, pending.has_key()).build(),
        Stage::Mailbox(_) => mailbox(signing).build(),
        Stage::Fresh | Stage::Opening => fresh(signing).build(),
    }
}

fn fresh(signing: &Signing) -> impl View {
    column! {
        text_field(&signing.username)
            .placeholder(tr!("proton.signing.username"))
            .autofocus(true)
            .enabled(!signing.busy)
            .on_submit(|desk: &mut Desk| desk.sign_in())
            .fill_width();
        text_field(&signing.password)
            .placeholder(tr!("proton.signing.password"))
            .secret(true)
            .enabled(!signing.busy)
            .on_submit(|desk: &mut Desk| desk.sign_in())
            .fill_width();
        button(text(tr!("proton.signing.sign-in")))
            .primary()
            .loading(signing.busy)
            .on_press(|desk: &mut Desk| desk.sign_in())
            .fill_width();
    }
    .gap(Space::SNUG)
    .fill_width()
}

fn locked(signing: &Signing, username: &str) -> impl View {
    column! {
        text(tr!("proton.signing.locked", name = username)).label().align(TextAlign::Center);
        text_field(&signing.password)
            .placeholder(tr!("proton.signing.password"))
            .secret(true)
            .autofocus(true)
            .enabled(!signing.busy)
            .on_submit(|desk: &mut Desk| desk.unlock())
            .fill_width();
        button(text(tr!("proton.signing.unlock")))
            .primary()
            .loading(signing.busy)
            .on_press(|desk: &mut Desk| desk.unlock())
            .fill_width();
        button(text(tr!("proton.signing.another")))
            .ghost()
            .on_press(|desk: &mut Desk| desk.forget_account())
            .fill_width();
    }
    .gap(Space::SNUG)
    .fill_width()
}

fn code(signing: &Signing, key: bool) -> impl View {
    column! {
        text(match key {
            true => tr!("proton.signing.code-or-key"),
            false => tr!("proton.signing.code"),
        })
        .label()
        .align(TextAlign::Center);
        text_field(&signing.code)
            .placeholder(tr!("proton.signing.code-hint"))
            .autofocus(true)
            .max_len(8)
            .enabled(!signing.busy)
            .on_submit(|desk: &mut Desk| desk.submit_code())
            .fill_width();
        button(text(tr!("proton.signing.go-on")))
            .primary()
            .loading(signing.busy)
            .on_press(|desk: &mut Desk| desk.submit_code())
            .fill_width();
    }
    .gap(Space::SNUG)
    .fill_width()
}

fn mailbox(signing: &Signing) -> impl View {
    column! {
        text(tr!("proton.signing.mailbox")).label().align(TextAlign::Center);
        text_field(&signing.mailbox)
            .placeholder(tr!("proton.signing.mailbox-hint"))
            .secret(true)
            .autofocus(true)
            .enabled(!signing.busy)
            .on_submit(|desk: &mut Desk| desk.submit_mailbox())
            .fill_width();
        button(text(tr!("proton.signing.go-on")))
            .primary()
            .loading(signing.busy)
            .on_press(|desk: &mut Desk| desk.submit_mailbox())
            .fill_width();
    }
    .gap(Space::SNUG)
    .fill_width()
}
