use browser_pages::{ErrorAction, FailureKind};
use rikka_app::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Presentation {
    pub(crate) symbol: MaterialSymbol,
    pub(crate) action: ErrorAction,
}

pub(crate) fn presentation(kind: FailureKind) -> Presentation {
    let symbol = match kind {
        FailureKind::Offline => MaterialSymbol::WIFI_OFF,
        FailureKind::Tls => MaterialSymbol::GPP_BAD,
        FailureKind::TooManyRedirects => MaterialSymbol::SYNC_PROBLEM,
        FailureKind::Blocked => MaterialSymbol::BLOCK,
        FailureKind::FileNotFound => MaterialSymbol::FILE_OPEN,
        FailureKind::UnsupportedScheme => MaterialSymbol::OPEN_IN_NEW,
        FailureKind::Content => MaterialSymbol::BROKEN_IMAGE,
        FailureKind::HostNotFound
        | FailureKind::ConnectionRefused
        | FailureKind::ConnectionReset
        | FailureKind::TimedOut
        | FailureKind::Proxy
        | FailureKind::Other => MaterialSymbol::CLOUD_OFF,
    };
    Presentation {
        symbol,
        action: kind.action(),
    }
}

pub(crate) fn title(kind: FailureKind) -> Label {
    match kind {
        FailureKind::Offline => tr!("browser.interstitial.offline-title"),
        FailureKind::Tls => tr!("browser.interstitial.tls-title"),
        FailureKind::TooManyRedirects | FailureKind::Content => {
            tr!("browser.interstitial.broken-title")
        }
        FailureKind::Blocked => tr!("browser.interstitial.blocked-title"),
        FailureKind::FileNotFound => tr!("browser.interstitial.file-title"),
        FailureKind::UnsupportedScheme => tr!("browser.interstitial.scheme-title"),
        FailureKind::HostNotFound
        | FailureKind::ConnectionRefused
        | FailureKind::ConnectionReset
        | FailureKind::TimedOut
        | FailureKind::Proxy
        | FailureKind::Other => tr!("browser.interstitial.network-title"),
    }
}

pub(crate) fn message(kind: FailureKind) -> Option<Label> {
    Some(match kind {
        FailureKind::HostNotFound => tr!("browser.interstitial.host-not-found"),
        FailureKind::ConnectionRefused => tr!("browser.interstitial.refused"),
        FailureKind::ConnectionReset => tr!("browser.interstitial.reset"),
        FailureKind::TimedOut => tr!("browser.interstitial.timed-out"),
        FailureKind::TooManyRedirects => tr!("browser.interstitial.redirects"),
        FailureKind::Proxy => tr!("browser.interstitial.proxy"),
        FailureKind::Offline
        | FailureKind::Tls
        | FailureKind::Blocked
        | FailureKind::FileNotFound
        | FailureKind::UnsupportedScheme
        | FailureKind::Content
        | FailureKind::Other => return None,
    })
}
