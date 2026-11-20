use rikka_ui::{
    Align, Element, Inset, Label, MaterialSymbol, Space, Styled, TextStyled, View, text,
};
use rikka_widgets::{column, row};
use shell_kit::clip;

use crate::centre::detail::Detail;
use crate::centre::state::Centre;
use crate::centre::view::{bluetooth, network, sound, tile};

pub(crate) fn view(centre: &Centre, page: Detail) -> impl View {
    let daemons = centre.daemons();
    let (radio, mut rows) = match page {
        Detail::Sound => (None, sound::playing(&daemons.sound().state())),
        Detail::Microphone => (None, sound::recording(&daemons.sound().state())),
        Detail::Network => {
            let network = daemons.network().state();
            (
                Some(network::radio(&network).build()),
                network::page(centre, &network),
            )
        }
        Detail::Bluetooth => {
            let bluetooth = daemons.bluetooth().state();
            (
                Some(bluetooth::radio(&bluetooth).build()),
                bluetooth::page(&bluetooth),
            )
        }
    };
    rows.push(tile::more(page).build());

    column! {
        heading(page.title(), radio);
        tile::tile(rows)
    }
    .gap(Space::SNUG)
    .align_items(Align::Stretch)
    .fill_width()
}

fn heading(title: Label, radio: Option<Element>) -> impl View {
    row! {
        tile::symbol_button(MaterialSymbol::ARROW_BACK, |centre: &mut Centre| centre.open(None));
        clip(text(title).body().medium().no_wrap());
        radio
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
    .padding(Inset::xy(tile::PADDING, 0.0))
}
