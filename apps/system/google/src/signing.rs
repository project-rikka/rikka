use google_account::SavedAccount;
use rikka_app::prelude::*;
use rikka_web::web_view;
use rikka_widgets::{column, row};

use crate::avatar::avatar;
use crate::google::{Google, Stage};
use crate::portal::Purpose;

const CARD: f32 = 420.0;
const MARK: f32 = 56.0;
const AVATAR: f32 = 32.0;

pub(crate) fn view(google: &Google) -> Element {
    match (google.stage(), google.portal()) {
        (Stage::Page(purpose), Some(portal)) => column! {
            row! {
                icon(BrandSymbol::GOOGLE).extent(Length::px(18.0)).color(palette().accent);
                text(match purpose {
                    Purpose::SignIn => tr!("google.signing.page"),
                    Purpose::VaultKeys => tr!("google.passwords.verify.page"),
                })
                .label()
                .semibold()
                .grow(1.0);
                button(text(tr!("google.cancel")))
                    .ghost()
                    .on_press(Google::cancel_portal);
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .padding(Inset::xy(Space::LOOSE, Space::SNUG))
            .fill_width();
            rule();
            web_view(portal.session()).grow(1.0).fill_width();
        }
        .fill()
        .any(),
        (Stage::Welcome, _) => welcome(google),
        _ => waiting(),
    }
}

fn waiting() -> Element {
    container(
        column! {
            spinner().extent(Length::px(28.0));
            text(tr!("google.signing.connecting")).label().muted();
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center),
    )
    .center()
    .fill()
    .any()
}

fn welcome(google: &Google) -> Element {
    container(
        column! {
            icon(BrandSymbol::GOOGLE).extent(Length::px(MARK)).color(palette().accent);
            text(tr!("google.title")).display().bold();
            text(tr!("google.signing.summary"))
                .muted()
                .align(TextAlign::Center)
                .fill_width();
            spacer().height(Space::SNUG.px());
            if !google.accounts().is_empty() {
                column_of(google.accounts().iter().map(saved)).gap(Space::TIGHT).fill_width();
            }
            button(
                row! {
                    icon(MaterialSymbol::LOGIN).extent(Length::px(18.0));
                    text(match google.accounts().is_empty() {
                        true => tr!("google.signing.start"),
                        false => tr!("google.signing.another"),
                    });
                }
                .gap(Space::SNUG)
                .align_items(Align::Center),
            )
            .primary()
            .large()
            .on_press(Google::sign_in);
            if let Some(failure) = google.failure() {
                text(failure.to_owned())
                    .caption()
                    .danger()
                    .align(TextAlign::Center)
                    .fill_width();
            }
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center)
        .width(CARD),
    )
    .center()
    .fill()
    .transition(fade_scale())
    .any()
}

fn saved(account: &SavedAccount) -> Element {
    let key = account.key.clone();
    container(
        row! {
            avatar(&account.profile, AVATAR);
            column! {
                text(account.profile.name.clone()).label().medium().no_wrap();
                text(account.profile.email.clone()).caption().muted().no_wrap();
            }
            .gap(Space::HAIR)
            .grow(1.0)
            .min_width(0.0);
            icon(MaterialSymbol::CHEVRON_RIGHT)
                .extent(Length::px(18.0))
                .color(palette().text_muted);
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center),
    )
    .padding(Inset::xy(Space::NORMAL, Space::SNUG))
    .radius(Corner::Item)
    .background(palette().surface_variant)
    .on_press(move |google: &mut Google| google.switch_to(&key))
    .fill_width()
    .any()
}
