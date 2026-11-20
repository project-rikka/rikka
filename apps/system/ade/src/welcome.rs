use std::path::PathBuf;

use rikka_app::prelude::*;
use rikka_widgets::column;

use crate::ade::{self, Ade};
use crate::labels;
use crate::notice::notice;
use crate::settings::Section;
use crate::setup::Setup;

const TILE: f32 = 84.0;
const GLYPH: f32 = 30.0;
const RECENTS: f32 = 380.0;

pub(crate) fn view(setup: &Setup) -> impl View {
    container(
        column! {
            head(setup);
            if !setup.config.recents().is_empty() {
                recents(setup);
            }
        }
        .gap(Space::ROOMY)
        .align_items(Align::Center)
        .max_height(Length::FILL),
    )
    .center()
    .fill()
    .padding(Space::WIDE)
}

fn head(setup: &Setup) -> impl View {
    column! {
        tile(MaterialSymbol::FOLDER_OPEN, tr!("ade.welcome.open"));
        text(tr!("ade.welcome.hint")).label().muted();
        if setup.llm.selected_account().is_none() {
            notice(MaterialSymbol::KEY, tr!("ade.welcome.signed-out"), palette().warning)
                .on_dismiss(tr!("ade.welcome.sign-in"), |ade: &mut Ade| {
                    ade.open_settings(Section::Llm);
                });
        }
    }
    .gap(Space::ROOMY)
    .align_items(Align::Center)
    .shrink(0.0)
}

fn tile(symbol: MaterialSymbol, label: Label) -> impl View {
    column! {
        button(icon(symbol).extent(Length::px(GLYPH)))
            .icon_only()
            .on_press(ade::ask)
            .width(TILE)
            .height(TILE);
        text(label).label().medium();
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
}

fn recents(setup: &Setup) -> impl View {
    let recents = setup.config.recents();
    let opening: Vec<PathBuf> = recents.to_vec();
    let rows = list(recents.iter().map(|project| {
        list_item(labels::file_name(project))
            .secondary(project.display().to_string())
            .icon(MaterialSymbol::FOLDER)
    }))
    .selected(None)
    .activates_on_press(true)
    .on_activate(
        move |ade: &mut Ade, index: usize| match opening.get(index) {
            Some(project) => ade.open(project.clone()),
            None => ade::ask(),
        },
    )
    .fill_width();
    column! {
        text(tr!("ade.welcome.recent")).caption().medium();
        rows;
    }
    .gap(Space::SNUG)
    .width(RECENTS)
}
