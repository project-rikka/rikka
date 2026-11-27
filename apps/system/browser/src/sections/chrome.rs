use browser_pages::ChromePage;
use rikka_app::prelude::*;
use rikka_web::{Account, ChromeSettingValue, PrivacySetting};
use rikka_widgets::row;

use crate::shell::Shell;
use browser_model::ProfileKey;

const READING: &str = "rikka-browser-privacy-read";
const WRITING: &str = "rikka-browser-privacy-write";

const TOGGLES: [PrivacySetting; 7] = [
    PrivacySetting::PasswordSaving,
    PrivacySetting::AutofillAddresses,
    PrivacySetting::AutofillCards,
    PrivacySetting::SafeBrowsing,
    PrivacySetting::ThirdPartyCookies,
    PrivacySetting::DoNotTrack,
    PrivacySetting::NetworkPrediction,
];

const LINKS: [ChromePage; 10] = [
    ChromePage::Account,
    ChromePage::Passwords,
    ChromePage::Payments,
    ChromePage::Addresses,
    ChromePage::SiteSettings,
    ChromePage::SearchEngines,
    ChromePage::Languages,
    ChromePage::Downloads,
    ChromePage::Security,
    ChromePage::Extensions,
];

fn toggle_name(setting: PrivacySetting) -> Label {
    match setting {
        PrivacySetting::PasswordSaving => tr!("browser.chrome.password-saving"),
        PrivacySetting::AutofillAddresses => tr!("browser.chrome.autofill-addresses"),
        PrivacySetting::AutofillCards => tr!("browser.chrome.autofill-cards"),
        PrivacySetting::SafeBrowsing => tr!("browser.chrome.safe-browsing"),
        PrivacySetting::ThirdPartyCookies => tr!("browser.chrome.third-party-cookies"),
        PrivacySetting::DoNotTrack => tr!("browser.chrome.do-not-track"),
        PrivacySetting::NetworkPrediction => tr!("browser.chrome.network-prediction"),
        PrivacySetting::SpellingService
        | PrivacySetting::TranslationService
        | PrivacySetting::HyperlinkAuditing => tr!("browser.chrome.settings"),
    }
}

fn link_name(page: ChromePage) -> (MaterialSymbol, Label) {
    match page {
        ChromePage::Account | ChromePage::Sync => (
            MaterialSymbol::ACCOUNT_CIRCLE,
            tr!("browser.chrome.account"),
        ),
        ChromePage::Passwords => (MaterialSymbol::PASSWORD, tr!("browser.chrome.passwords")),
        ChromePage::Payments => (MaterialSymbol::CREDIT_CARD, tr!("browser.chrome.payments")),
        ChromePage::Addresses | ChromePage::Autofill => {
            (MaterialSymbol::HOME_PIN, tr!("browser.chrome.addresses"))
        }
        ChromePage::SiteSettings => (MaterialSymbol::TUNE, tr!("browser.chrome.site-settings")),
        ChromePage::SearchEngines => (MaterialSymbol::SEARCH, tr!("browser.chrome.search-engines")),
        ChromePage::Languages => (MaterialSymbol::LANGUAGE, tr!("browser.chrome.languages")),
        ChromePage::Downloads => (MaterialSymbol::DOWNLOAD, tr!("browser.chrome.downloads")),
        ChromePage::Security | ChromePage::Privacy => {
            (MaterialSymbol::SECURITY, tr!("browser.chrome.security"))
        }
        ChromePage::Extensions => (MaterialSymbol::EXTENSION, tr!("browser.chrome.extensions")),
        ChromePage::Settings | ChromePage::History | ChromePage::Bookmarks => {
            (MaterialSymbol::SETTINGS, tr!("browser.chrome.settings"))
        }
    }
}

pub(crate) fn view(shell: &Shell) -> Element {
    let key = shell.model().profile_here(shell.id());
    let chrome = &shell.browser().services().chrome;
    let toggles = TOGGLES.map(|setting| {
        let value = chrome.privacy(key, setting);
        let control = switch(value.is_some_and(|value| value.value))
            .enabled(value.is_some_and(|value| value.is_changeable()))
            .on_toggle(move |shell: &mut Shell, on: bool| shell.set_privacy(setting, on));
        field(toggle_name(setting), control).any()
    });
    let links = LINKS.map(|page| {
        let (symbol, title) = link_name(page);
        container(
            row! {
                icon(symbol).extent(Length::px(18.0)).color(palette().text_muted);
                text(title).label().grow(1.0).min_width(0.0);
                icon(MaterialSymbol::OPEN_IN_NEW).extent(Length::px(16.0)).color(palette().text_muted);
            }
            .gap(Space::NORMAL)
            .align_items(Align::Center),
        )
        .padding(Inset::xy(Space::SNUG, Space::SNUG))
        .radius(Corner::Item)
        .on_press(move |shell: &mut Shell| shell.open_page(page.address().to_owned()))
        .fill_width()
        .any()
    });
    page([
        section(tr!("browser.chrome.account"), account_row(shell)),
        section(
            tr!("browser.chrome.pages"),
            column_of(links).gap(Space::HAIR).fill_width(),
        ),
        section(
            tr!("browser.chrome.privacy"),
            column_of(toggles).gap(Space::NORMAL).fill_width(),
        ),
    ])
    .any()
}

pub(crate) fn account_row(shell: &Shell) -> Element {
    let key = shell.model().profile_here(shell.id());
    if key == ProfileKey::Private {
        return text(tr!("browser.chrome.private-account"))
            .muted()
            .fill_width()
            .any();
    }
    let account: Option<Account> = shell.browser().services().chrome.account(key);
    let (headline, action) = match &account {
        Some(account) => (account.email.clone(), tr!("browser.chrome.manage-sync")),
        None => (
            tr!("browser.chrome.signed-out").to_string(),
            tr!("browser.chrome.turn-on-sync"),
        ),
    };
    row! {
        icon(MaterialSymbol::ACCOUNT_CIRCLE).extent(Length::px(28.0)).color(palette().text_muted);
        text(headline).label().no_wrap().grow(1.0).min_width(0.0);
        button(text(action))
            .secondary()
            .small()
            .on_press(|shell: &mut Shell| shell.open_page(ChromePage::Sync.address().to_owned()));
    }
    .gap(Space::NORMAL)
    .align_items(Align::Center)
    .fill_width()
    .any()
}

impl Shell {
    pub(crate) fn read_privacy(&mut self, key: ProfileKey) {
        let privacy = match self.browser().engine(key) {
            Ok(runtime) => runtime.privacy(),
            Err(error) => {
                tracing::warn!(%error, "failed to start Chrome for its settings");
                return;
            }
        };
        for setting in TOGGLES {
            spawn_keyed(
                (READING, key, setting),
                privacy.get(setting),
                move |shell: &mut Shell, read: rikka_web::Result<ChromeSettingValue>| match read {
                    Ok(value) => shell
                        .browser()
                        .services()
                        .chrome
                        .set_privacy(key, setting, value),
                    Err(error) => {
                        tracing::debug!(%error, ?setting, "failed to read a Chrome setting")
                    }
                },
            );
        }
    }

    fn set_privacy(&mut self, setting: PrivacySetting, on: bool) {
        let key = self.model().profile_here(self.id());
        let Some(runtime) = self.browser().running_engine(key) else {
            return;
        };
        spawn_keyed(
            (WRITING, key, setting),
            runtime.privacy().set(setting, on),
            move |shell: &mut Shell, written: rikka_web::Result<()>| {
                if let Err(error) = written {
                    tracing::warn!(%error, ?setting, "failed to change a Chrome setting");
                }
                shell.read_privacy(key);
            },
        );
    }
}
