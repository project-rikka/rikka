use rikka_app::prelude::*;
use rikka_settings::session::lock::{
    AT_LOGIN, AT_LOGIN_DEFAULT, BEFORE_SLEEP, BEFORE_SLEEP_DEFAULT, DOMAIN, IDLE, IDLE_DEFAULT,
    IDLES,
};
use rikka_widgets::column;

use crate::controls::{Count, flag_or, span};
use crate::domain::{Domain, domain_page};
use crate::notice::domain_notice;
use crate::page::Page;
use crate::pending::Pending;
use crate::session::Session;

pub(crate) struct Lock {
    domain: Domain,
}

impl Page for Lock {
    fn opened(&mut self, _: &Pending, session: &Session) {
        self.domain.open::<Self>(session);
    }
}

domain_page!(Lock);

impl Lock {
    pub(crate) fn new() -> Self {
        Self {
            domain: Domain::new(DOMAIN),
        }
    }
}

pub(crate) fn view(lock: &Lock) -> impl View {
    column! {
        domain_notice(
            &lock.domain,
            "settings.lock-screen.unavailable",
            "settings.lock-screen.refused",
        );
        column! {
            Count {
                key: IDLE,
                absent: IDLE_DEFAULT,
                range: span(&IDLES),
            }
            .field(lock, tr!("settings.lock-screen.idle"), |minutes| match minutes {
                0 => tr!("settings.lock-screen.never").to_string(),
                minutes => tr!("settings.lock-screen.minutes", count = minutes).to_string(),
            });
            flag_or(
                lock,
                tr!("settings.lock-screen.before-sleep"),
                BEFORE_SLEEP,
                BEFORE_SLEEP_DEFAULT,
            );
            flag_or(
                lock,
                tr!("settings.lock-screen.at-login"),
                AT_LOGIN,
                AT_LOGIN_DEFAULT,
            );
        }
        .gap(Space::SNUG);
    }
    .gap(Space::ROOMY)
}

impl std::fmt::Debug for Lock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Lock").finish_non_exhaustive()
    }
}
