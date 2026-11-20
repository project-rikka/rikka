use rikka_app::prelude::*;
use rikka_settings::session::notifications::{
    DO_NOT_DISTURB, DOMAIN, TIMEOUT, TIMEOUT_DEFAULT, TIMEOUTS, VISIBLE, VISIBLE_DEFAULT, VISIBLES,
};
use rikka_widgets::column;

use crate::controls::{Count, flag};
use crate::domain::{Domain, domain_page};
use crate::notice::domain_notice;
use crate::page::Page;
use crate::pending::Pending;
use crate::session::Session;

const LONGEST: f32 = *TIMEOUTS.end() as f32;
const MOST: f32 = *VISIBLES.end() as f32;

pub(crate) struct Notifications {
    domain: Domain,
}

impl Page for Notifications {
    fn opened(&mut self, _: &Pending, session: &Session) {
        self.domain.open::<Self>(session);
    }
}

domain_page!(Notifications);

impl Notifications {
    pub(crate) fn new() -> Self {
        Self {
            domain: Domain::new(DOMAIN),
        }
    }

    fn do_not_disturb(&self) -> bool {
        self.domain.get(DO_NOT_DISTURB).unwrap_or(false)
    }
}

pub(crate) fn view(notifications: &Notifications) -> impl View {
    column! {
        domain_notice(
            &notifications.domain,
            "settings.notifications.unavailable",
            "settings.notifications.refused",
        );
        controls(notifications);
    }
    .gap(Space::ROOMY)
}

fn controls(notifications: &Notifications) -> impl View {
    column! {
        flag(
            notifications,
            tr!("settings.notifications.do-not-disturb"),
            DO_NOT_DISTURB,
        );
        if !notifications.do_not_disturb() {
            Count {
                key: TIMEOUT,
                absent: TIMEOUT_DEFAULT,
                range: 1.0..=LONGEST,
            }
            .field(
                notifications,
                tr!("settings.notifications.timeout"),
                |seconds| tr!("settings.notifications.seconds", count = seconds).to_string(),
            );
        }
        Count {
            key: VISIBLE,
            absent: VISIBLE_DEFAULT,
            range: 1.0..=MOST,
        }
        .field(
            notifications,
            tr!("settings.notifications.visible"),
            |count| count.to_string(),
        );
    }
    .gap(Space::SNUG)
}

impl std::fmt::Debug for Notifications {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Notifications")
            .field("do-not-disturb", &self.do_not_disturb())
            .finish_non_exhaustive()
    }
}
