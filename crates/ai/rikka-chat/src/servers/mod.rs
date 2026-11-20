mod card;
mod draft;
mod form;
mod offered;
mod offers;
mod state;
mod trial;

use rikka_widgets::{column, row};

use crate::prelude::*;

pub use state::Servers;

use card::card;
use form::form;

pub type Titled = fn(&str) -> Label;

const GLYPH: f32 = 18.0;

pub fn servers(servers: &Servers, titled: Titled) -> impl View {
    let switching = servers.places().len() > 1;
    column! {
        if switching {
            segmented(servers.scope())
                .segments(servers.places().iter().map(|place| titled(place.name())))
                .on_select(|servers: &mut Servers, at: usize| servers.look(at));
        }
        page_stack(servers.scope(), scoped(servers, titled)).fill_width();
    }
    .gap(Space::ROOMY)
    .fill_width()
}

fn scoped(servers: &Servers, titled: Titled) -> impl View {
    column! {
        listed(servers, titled);
        text(servers.place().path().display().to_string()).caption().muted();
        for trouble in servers.troubles() {
            banner(trouble.to_owned())
                .symbol(MaterialSymbol::ERROR)
                .tint(palette().warning);
        }
    }
    .gap(Space::ROOMY)
    .fill_width()
}

fn listed(servers: &Servers, titled: Titled) -> Container {
    let draft = servers.draft();
    let checked = servers.checked();
    let rows = match servers.declared() {
        Err(_) => column! {
            text(tr!("chat.servers.unreadable")).caption().muted();
        },
        Ok(declared) => {
            let adding =
                draft.filter(|draft| draft.was().is_none_or(|was| declared.get(was).is_none()));
            column! {
                if declared.is_empty() && adding.is_none() {
                    text(tr!("chat.servers.none")).caption().muted();
                }
                for (nth, server) in declared.servers().iter().enumerate() {
                    if nth > 0 {
                        rule();
                    }
                    if let (Some(draft), Some(checked)) = (
                        draft.filter(|draft| draft.was() == Some(server.name())),
                        checked.as_ref(),
                    ) {
                        form(draft, checked);
                    } else {
                        card(server, servers, titled);
                    }
                }
                if let (Some(draft), Some(checked)) = (adding, checked.as_ref()) {
                    if !declared.is_empty() {
                        rule();
                    }
                    form(draft, checked);
                } else {
                    adder(draft.is_none());
                }
            }
        }
    }
    .gap(Space::NORMAL)
    .fill_width();

    section(tr!("chat.servers.declared"), rows)
}

fn adder(enabled: bool) -> impl View {
    button(
        row! {
            icon(MaterialSymbol::ADD).extent(Length::px(GLYPH));
            text(tr!("chat.servers.add")).caption();
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center),
    )
    .secondary()
    .small()
    .enabled(enabled)
    .on_press(Servers::add)
}
