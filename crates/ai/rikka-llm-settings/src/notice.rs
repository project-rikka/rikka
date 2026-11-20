use crate::prelude::*;

pub(crate) fn notice(banner: Banner) -> Container {
    container(banner)
        .surface()
        .radius(Corner::Panel)
        .padding(Space::NORMAL)
}
