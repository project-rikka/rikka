use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::avatar::avatar;
use crate::drive::{self, DriveState};
use crate::google::{Google, Section, Stage};
use crate::mail::{self, MailState};
use crate::passwords::{self, PasswordState};
use crate::signing;

pub(crate) const STAGE: &str = "google-stage";

const RAIL: f32 = 76.0;
const RAIL_ITEM: f32 = 60.0;
const RAIL_ICON: f32 = 24.0;
const ACCOUNT: &str = "google-account";
const ACCOUNT_MENU: &str = "google-account-menu";
const ACCOUNT_MENU_WIDTH: f32 = 300.0;
const AVATAR: f32 = 36.0;

pub(crate) fn caption_bar(_: &Google) -> Option<CaptionBar> {
    Some(
        caption()
            .leading(
                row! {
                    icon(BrandSymbol::GOOGLE).extent(Length::px(15.0));
                    text(tr!("google.title")).label().medium().no_wrap();
                }
                .gap(Space::SNUG)
                .align_items(Align::Center),
            )
            .into(),
    )
}

pub(crate) fn view(google: &Google) -> Element {
    match google.stage() {
        Stage::Ready => ready(google),
        Stage::Starting | Stage::Welcome | Stage::Page(_) | Stage::Connecting => {
            component(|google: &Google| google, signing::view)
        }
    }
}

fn ready(google: &Google) -> Element {
    let section = google.section();
    let body = column! {
        if google.needs_sign_in() {
            banner(tr!("google.signing.expired"))
                .symbol(MaterialSymbol::LOGIN)
                .on_action(tr!("google.signing.again"), Google::sign_in);
        }
        if let Some(failure) = google.failure() {
            banner(failure.to_owned())
                .on_dismiss(tr!("google.dismiss"), Google::dismiss_failure);
        }
        row! {
            rail(google);
            rule_upright();
            page_stack(section.index(), pane(section))
                .grow(1.0)
                .fill_height();
        }
        .grow(1.0)
        .fill_width();
    }
    .fill();
    let mut layers = vec![body.build().keyed(STAGE)];
    if google.account_menu() {
        layers.push(account_menu(google));
    }
    column_of(layers).fill().any()
}

fn pane(section: Section) -> Element {
    match section {
        Section::Mail => component(|mail: &MailState| mail, mail::view::view),
        Section::Drive => component(|drive: &DriveState| drive, drive::view::view),
        Section::Passwords => {
            component(|passwords: &PasswordState| passwords, passwords::view::view)
        }
    }
}

fn rail(google: &Google) -> impl View {
    let current = google.section();
    column! {
        for section in Section::ALL {
            destination(section, section == current);
        }
        spacer().grow(1.0);
        if let Some(account) = google.current() {
            container(avatar(&account.profile, AVATAR))
                .center()
                .height(RAIL_ITEM)
                .fill_width()
                .on_press(Google::toggle_account_menu)
                .build()
                .keyed(ACCOUNT)
                .tooltip(account.profile.email.clone());
        }
    }
    .gap(Space::TIGHT)
    .padding(Inset::xy(Space::SNUG, Space::SNUG))
    .width(RAIL)
    .fill_height()
}

fn destination(section: Section, current: bool) -> Element {
    let palette = palette();
    let (ink, fill) = match current {
        true => (palette.accent, palette.selection),
        false => (palette.text_muted, Color::TRANSPARENT),
    };
    let mark = icon(section.symbol())
        .extent(Length::px(RAIL_ICON))
        .color(ink);
    container(
        column! {
            match current {
                true => mark.filled(),
                false => mark,
            };
            text(section.title()).caption().color(ink).no_wrap();
        }
        .gap(Space::HAIR)
        .align_items(Align::Center),
    )
    .center()
    .height(RAIL_ITEM)
    .fill_width()
    .radius(Corner::Item)
    .background(fill)
    .on_press(move |google: &mut Google| google.select(section))
    .build()
    .animate(Motion::Standard)
}

fn account_menu(google: &Google) -> Element {
    let current = google.current().map(|account| account.key.clone());
    let mut items = Vec::new();
    for account in google.accounts() {
        let key = account.key.clone();
        items.push(
            menu_item(account.profile.email.clone())
                .icon(MaterialSymbol::ACCOUNT_CIRCLE)
                .selected(current.as_deref() == Some(key.as_str()))
                .on_press(move |google: &mut Google| google.switch_to(&key)),
        );
    }
    items.push(menu_separator());
    items.push(
        menu_item(tr!("google.account.add"))
            .icon(MaterialSymbol::PERSON_ADD)
            .on_press(Google::sign_in),
    );
    if let Some(key) = current {
        items.push(
            menu_item(tr!("google.account.sign-out"))
                .icon(MaterialSymbol::LOGOUT)
                .on_press(move |google: &mut Google| google.forget(&key)),
        );
    }
    menu()
        .items(items)
        .width(ACCOUNT_MENU_WIDTH)
        .on_dismiss(Google::close_account_menu)
        .build()
        .keyed(ACCOUNT_MENU)
        .anchored(Anchor::above(ACCOUNT))
}
