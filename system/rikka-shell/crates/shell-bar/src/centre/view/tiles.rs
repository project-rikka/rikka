use rikka_ui::{Align, Space, Styled, View};
use rikka_widgets::{column, grid_of};

use crate::centre::state::Centre;
use crate::centre::view::{
    Readings, airplane, bluetooth, header, network, notifications, quiet, sound,
};

const COLUMNS: usize = 2;

pub(crate) fn view(centre: &Centre) -> impl View {
    let readings = Readings::from_daemons(centre.daemons());
    column! {
        header::view(&readings);
        grid_of(COLUMNS, tiles(centre, &readings))
            .gap(Space::SNUG)
            .fill_width();
        notifications::view(centre)
    }
    .gap(Space::SNUG)
    .align_items(Align::Stretch)
    .fill_width()
}

fn tiles(centre: &Centre, readings: &Readings) -> [impl View; 6] {
    [
        network::tile(&readings.network),
        bluetooth::tile(&readings.bluetooth),
        sound::speaker(&readings.graph),
        sound::microphone(&readings.graph),
        airplane::tile(&readings.network, &readings.bluetooth),
        quiet::tile(centre.is_quiet()),
    ]
}
