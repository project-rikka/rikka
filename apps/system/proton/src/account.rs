use proton_config::Config;
use rikka_app::prelude::*;
use rikka_proton_client::Session;
use rikka_widgets::{column, row};

use crate::desk::Desk;
use crate::{pane, wording};

#[derive(Debug, State)]
pub(crate) struct Account {
    session: Session,
    leaving: bool,
    cleared: bool,
}

impl Account {
    pub(crate) fn new(session: Session) -> Account {
        Account {
            session,
            leaving: false,
            cleared: false,
        }
    }

    pub(crate) fn opened(&mut self) {
        match self.cleared {
            true => self.cleared = false,
            false => skip_view(),
        }
    }

    pub(crate) fn describe(&mut self, session: Session) {
        self.session = session;
    }

    pub(crate) fn leaving(&mut self) {
        self.leaving = true;
    }

    pub(crate) fn forgot(&mut self) {
        self.cleared = true;
    }
}

pub(crate) fn view(account: &Account, config: &Config) -> impl View {
    let keys = account.session.keys();
    let user = keys.user();
    let used = user.used_space();
    let most = user.max_space().max(1);
    let share = (used as f64 / most as f64) as f32;

    page([
        section(
            tr!("proton.account.who"),
            column! {
                row! {
                    pane::avatar(
                        &wording::initial(
                            user.display_name().or_else(|| user.name()).unwrap_or("?"),
                        ),
                        palette().accent,
                    );
                    column! {
                        text(user.display_name().or_else(|| user.name()).unwrap_or_default().to_owned())
                            .label()
                            .semibold();
                        text(user.email().unwrap_or_default().to_owned()).caption().muted();
                    }
                    .gap(Space::HAIR)
                    .grow(1.0);
                }
                .gap(Space::NORMAL)
                .align_items(Align::Center)
                .fill_width();
                pane::fact(tr!("proton.account.addresses"), wording::count(keys.addresses().len()));
                pane::fact(
                    tr!("proton.account.keys"),
                    wording::count(keys.unlocked_key_count()),
                );
            }
            .gap(Space::NORMAL)
            .fill_width(),
        ),
        section(
            tr!("proton.account.storage"),
            column! {
                gauge(share).fill_width();
                text(tr!(
                    "proton.account.storage-said",
                    used = &wording::size(used as i64),
                    all = &wording::size(most as i64),
                ))
                .caption()
                .muted();
            }
            .gap(Space::SNUG)
            .fill_width(),
        ),
        section(
            tr!("proton.account.mail-addresses"),
            list(keys.addresses().iter().map(|address| {
                list_item(address.email().to_owned())
                    .secondary(match address.can_send() {
                        true => tr!("proton.account.can-send"),
                        false => tr!("proton.account.cannot-send"),
                    })
                    .icon(MaterialSymbol::ALTERNATE_EMAIL)
                    .keyed(address.id().as_str())
            }))
            .fill_width(),
        ),
        section(
            tr!("proton.account.this-machine"),
            column! {
                field(
                    tr!("proton.account.stay"),
                    switch(config.stay_signed_in())
                        .on_toggle(|desk: &mut Desk, keep: bool| desk.keep_key(keep)),
                );
                field(
                    tr!("proton.account.conversations"),
                    switch(config.conversations()).on_toggle(|desk: &mut Desk, grouped: bool| {
                        desk.set_conversations(grouped);
                    }),
                );
                field(
                    tr!("proton.account.tiles"),
                    switch(config.drive_tiles())
                        .on_toggle(|desk: &mut Desk, tiles: bool| desk.set_tiles(tiles)),
                );
                row! {
                    button(text(tr!("proton.account.clear-cache")))
                        .secondary()
                        .on_press(|desk: &mut Desk| desk.clear_cache());
                    if account.cleared {
                        text(tr!("proton.account.cleared")).caption().muted();
                    }
                    spacer().grow(1.0);
                }
                .gap(Space::SNUG)
                .align_items(Align::Center)
                .fill_width();
            }
            .gap(Space::NORMAL)
            .fill_width(),
        ),
        section(
            tr!("proton.account.leaving"),
            row! {
                button(text(tr!("proton.account.sign-out")))
                    .danger()
                    .loading(account.leaving)
                    .on_press(|desk: &mut Desk| desk.sign_out());
                spacer().grow(1.0);
                text(tr!(
                    "proton.account.since",
                    at = &wording::whole(user.create_time())
                ))
                .caption()
                .muted();
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width(),
        ),
    ])
    .fill()
    .build()
}
