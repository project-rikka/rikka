use rikka_ui::{Styled, palette};

use crate::blocks::container::{Container, container};
use crate::blocks::spacer::spacer;

const HAIRLINE: f32 = 1.0;

pub fn rule() -> Container {
    hairline().height(HAIRLINE).fill_width()
}

pub fn rule_upright() -> Container {
    hairline().width(HAIRLINE).fill_height()
}

fn hairline() -> Container {
    container(spacer()).background(palette().outline)
}
