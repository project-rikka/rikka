use rikka_accounts::AccountKind;
use rikka_app::prelude::*;
use rikka_dirs::passwd::Entry;
use rikka_nix::Module;
use rikka_widgets::{column, row};

use crate::notice::{acting_notice, notice};
use crate::pending::Pending;
use crate::users::component::{Link, Report, Users};

const FACE: f32 = 88.0;
const INITIAL: f32 = 0.42;
const ACCOUNTS_DAEMON: &str = "services.accounts-daemon.enable";

pub(crate) fn view(users: &Users) -> impl View {
    let editable = users.accounts().is_some() && !users.busy;
    section(
        tr!("settings.users.account"),
        column! {
            row! {
                face(users);
                column! {
                    text(Label::from(users.who().map_or("", Entry::display_name)))
                        .heading()
                        .semibold();
                    text(Label::from(users.who().map_or("", |who| who.name.as_str())))
                        .label()
                        .muted();
                    users.kind().map(|kind| text(kind_named(kind)).caption().muted());
                }
                .gap(Space::TIGHT)
                .grow(1.0);
            }
            .gap(Space::ROOMY)
            .align_items(Align::Center)
            .fill_width();
            row! {
                button(tr!("settings.users.choose-picture"))
                    .secondary()
                    .loading(users.busy)
                    .enabled(editable)
                    .on_press(Users::choose_picture);
                if users.has_picture() {
                    button(tr!("settings.users.remove-picture"))
                        .ghost()
                        .enabled(editable)
                        .on_press(Users::remove_picture);
                }
            }
            .gap(Space::SNUG);
        }
        .gap(Space::NORMAL),
    )
}

fn face(users: &Users) -> Element {
    let round = FACE / 2.0;
    match users.face() {
        Some(source) => image(source)
            .fit(ImageFit::Cover)
            .radius(round)
            .width(Length::px(FACE))
            .height(Length::px(FACE))
            .build(),
        None => container(
            text(Label::from(
                users.who().map(Entry::initial).unwrap_or_default(),
            ))
            .text_size(FACE * INITIAL)
            .semibold()
            .color(palette().on_accent),
        )
        .center()
        .background(palette().accent)
        .radius(round)
        .width(Length::px(FACE))
        .height(Length::px(FACE))
        .build(),
    }
}

fn kind_named(kind: AccountKind) -> Label {
    match kind {
        AccountKind::Administrator => tr!("settings.users.administrator"),
        AccountKind::Standard => tr!("settings.users.standard"),
    }
}

pub(crate) fn link_notice(users: &Users, pending: &Pending) -> Option<Element> {
    match users.link() {
        Link::Connecting | Link::Ready(_) => None,
        Link::Failed(reason) => Some(
            notice(
                MaterialSymbol::CLOUD_OFF,
                palette().warning,
                tr!("settings.users.accounts-unavailable", reason = reason),
            )
            .build(),
        ),
        Link::Missing if pending.get::<bool>(Module::Desktop, ACCOUNTS_DAEMON) == Some(true) => {
            Some(
                notice(
                    MaterialSymbol::PERSON,
                    palette().accent,
                    tr!("settings.users.accounts-staged"),
                )
                .build(),
            )
        }
        Link::Missing if pending.unavailable().is_none() => Some(
            acting_notice(
                MaterialSymbol::PERSON,
                palette().warning,
                tr!("settings.users.accounts-missing"),
                tr!("settings.users.enable-accounts"),
                |pending: &mut Pending| pending.set(Module::Desktop, ACCOUNTS_DAEMON, true),
            )
            .build(),
        ),
        Link::Missing => Some(
            notice(
                MaterialSymbol::PERSON,
                palette().warning,
                tr!("settings.users.accounts-missing"),
            )
            .build(),
        ),
    }
}

pub(crate) fn report_notice(report: &Report) -> impl View {
    let (symbol, ink, message) = match report {
        Report::PasswordChanged => (
            MaterialSymbol::CHECK_CIRCLE,
            palette().success,
            tr!("settings.users.password-changed"),
        ),
        Report::KeyringBehind(reason) => (
            MaterialSymbol::WARNING,
            palette().warning,
            tr!("settings.users.keyring-behind", reason = reason),
        ),
        Report::PictureUnreadable(reason) => (
            MaterialSymbol::ERROR,
            palette().danger,
            tr!("settings.users.picture-unreadable", reason = reason),
        ),
        Report::PictureRefused(reason) => (
            MaterialSymbol::ERROR,
            palette().danger,
            tr!("settings.users.picture-refused", reason = reason),
        ),
        Report::AccountsLost => (
            MaterialSymbol::CLOUD_OFF,
            palette().danger,
            tr!("settings.users.accounts-lost"),
        ),
    };
    acting_notice(
        symbol,
        ink,
        message,
        tr!("settings.users.dismiss"),
        Users::dismiss,
    )
}
