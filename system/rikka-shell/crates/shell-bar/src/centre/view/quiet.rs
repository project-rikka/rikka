use rikka_ui::{MaterialSymbol, tr};

use crate::centre::state::Centre;
use crate::centre::view::quick::{Quick, quick};

pub(crate) fn tile(quiet: bool) -> Quick {
    quick(symbol(quiet), tr!("shell.centre.quiet"))
        .on(quiet)
        .toggle(move |centre: &mut Centre| centre.quiet(!quiet))
}

fn symbol(quiet: bool) -> MaterialSymbol {
    match quiet {
        true => MaterialSymbol::NOTIFICATIONS_OFF,
        false => MaterialSymbol::NOTIFICATIONS,
    }
}
