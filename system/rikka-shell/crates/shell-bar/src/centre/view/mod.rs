mod airplane;
mod bluetooth;
mod header;
mod network;
mod notifications;
mod page;
mod power;
mod quick;
mod quiet;
mod sound;
mod tile;
mod tiles;

use rikka_battery::Reading;
use rikka_bluetooth::State as BluetoothState;
use rikka_network::State as NetworkState;
use rikka_power::State as PowerState;
use rikka_theme::Corner;
use rikka_types::{Key, Modifiers};
use rikka_ui::{
    Align, Curve, DurationToken, Inset, Motion, Property, Space, Spring, Styled, View, fade,
    is_exiting, offset, palette, scale,
};
use rikka_widgets::{column, container, page_stack};
use shell_feeds::Daemons;
use shell_kit::{scrim, sheet};

use crate::centre;
use crate::centre::state::Centre;

const RADIUS: f32 = Corner::Window.radius();
const BORDER: f32 = 1.0;

const DROP: f32 = 14.0;
const ENTER_FROM: f32 = 0.96;

pub(crate) const EXIT_DURATION: DurationToken = DurationToken::Short4;

pub(crate) fn view(centre: &Centre) -> impl View {
    scrim(
        column! {
            if !is_exiting() {
                sheet(card(centre), escape)
                    .build()
                    .width(centre::WIDTH)
                    .transition(
                        (fade() + scale(ENTER_FROM) + offset(0.0, -DROP))
                            .enter(Curve::Spring(Spring::DEFAULT_SPATIAL))
                            .exit(Motion::exit(EXIT_DURATION).curve(Property::Effects)),
                    );
            }
        }
        .align_items(Align::End)
        .padding(Inset::edges(
            centre.below() + centre::MARGIN,
            centre::MARGIN,
            0.0,
            0.0,
        ))
        .fill(),
        Centre::dismiss,
    )
}

fn escape(centre: &mut Centre, key: Key, _modifiers: Modifiers) -> bool {
    match (key, centre.page()) {
        (Key::Escape, Some(_)) => centre.open(None),
        (Key::Escape, None) => centre.dismiss(),
        _ => return false,
    }
    true
}

pub(crate) struct Readings {
    pub(crate) graph: rikka_sound::Snapshot,
    pub(crate) network: NetworkState,
    pub(crate) bluetooth: BluetoothState,
    pub(crate) power: PowerState,
    pub(crate) battery: Option<Reading>,
}

impl Readings {
    fn from_daemons(daemons: &Daemons) -> Readings {
        Readings {
            graph: daemons.sound().state(),
            network: daemons.network().state(),
            bluetooth: daemons.bluetooth().state(),
            power: daemons.power().state(),
            battery: daemons.battery().main(),
        }
    }
}

fn card(centre: &Centre) -> impl View {
    let palette = palette();
    let page = centre.page();
    let body = match page {
        Some(detail) => page::view(centre, detail).any(),
        None => tiles::view(centre).any(),
    };

    container(
        page_stack(usize::from(page.is_some()), body)
            .named(page)
            .fill_width(),
    )
    .background(palette.background)
    .border(BORDER, palette.outline)
    .radius(RADIUS)
    .padding(Space::SNUG)
    .glass()
    .resizing()
}
