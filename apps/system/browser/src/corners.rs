use rikka_app::prelude::*;

use crate::content;
use crate::shell::Shell;

const MARGIN: f32 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Corners {
    Rounded,
    Square,
}

impl Corners {
    pub(crate) fn for_shell(shell: &Shell) -> Corners {
        match shell.browser().preferences().rounded_corners() {
            true => Corners::Rounded,
            false => Corners::Square,
        }
    }

    pub(crate) fn pill(self, height: f32, square: f32) -> f32 {
        match self {
            Corners::Rounded => height / 2.0,
            Corners::Square => square,
        }
    }

    pub(crate) fn sidebar(self) -> Color {
        match self {
            Corners::Rounded => palette().background,
            Corners::Square => palette().surface,
        }
    }
}

pub(crate) fn page(shell: &Shell) -> Element {
    let framed = Corners::for_shell(shell) == Corners::Rounded && !shell.overlays.hides_chrome();
    let card = container(content::column()).fill();
    let (card, margin) = match framed {
        true => (
            card.radius(card_corners()).clip_content(),
            Inset::edges(0.0, MARGIN, MARGIN, 0.0),
        ),
        false => (card, Inset::NONE),
    };
    container(card).padding(margin).fill().any()
}

fn card_corners() -> CornerRadii {
    CornerRadii {
        bottom_right: Corner::Window.nested(MARGIN),
        ..CornerRadii::uniform(Corner::Panel.radius())
    }
}
