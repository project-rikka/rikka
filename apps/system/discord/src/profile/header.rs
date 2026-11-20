use rikka_app::prelude::*;
use rikka_ui::Handler;

use super::look::Look;
use crate::pictures;
use crate::widgets::avatar_with_status;

const ACTION: f32 = 30.0;
const ACTION_ICON: f32 = 18.0;

#[derive(Clone, Copy, Debug)]
pub(super) struct Frame {
    pub(super) width: f32,
    pub(super) banner: f32,
    pub(super) strip: f32,
    pub(super) face: f32,
    pub(super) ring: f32,
    pub(super) inset: f32,
    pub(super) corner: f32,
}

impl Frame {
    pub(super) const POPOUT: Frame = Frame {
        width: 300.0,
        banner: 105.0,
        strip: 60.0,
        face: 80.0,
        ring: 6.0,
        inset: 16.0,
        corner: Corner::Panel.radius(),
    };

    pub(super) const FULL: Frame = Frame {
        width: 600.0,
        banner: 170.0,
        strip: 100.0,
        face: 120.0,
        ring: 8.0,
        inset: 24.0,
        corner: Corner::Panel.radius(),
    };

    pub(super) fn panel(width: f32) -> Frame {
        Frame {
            width,
            banner: (width * 0.35).round(),
            corner: 0.0,
            ..Frame::POPOUT
        }
    }

    fn top(self, banner: bool) -> f32 {
        if banner { self.banner } else { self.strip }
    }

    fn ringed(self) -> f32 {
        self.face + 2.0 * self.ring
    }

    fn height(self, banner: bool) -> f32 {
        self.top(banner) + self.ringed() / 2.0
    }
}

pub(super) fn header(
    look: &Look<'_>,
    frame: Frame,
    actions: Vec<Element>,
    on_open: Option<Handler<()>>,
) -> Element {
    let picture = look
        .banner()
        .and_then(|banner| pictures::fetched(banner, frame.width));
    let has_banner = look.banner().is_some();
    let top = frame.top(has_banner);
    let rounded = CornerRadii::new(frame.corner, frame.corner, 0.0, 0.0);
    let backdrop = look.backdrop();
    let banner = match picture {
        Some(bytes) => image(bytes)
            .fit(ImageFit::Cover)
            .radius(rounded)
            .size(Length::FILL, Length::px(top))
            .any(),
        None => container(spacer())
            .background(look.banner_color())
            .radius(rounded)
            .size(Length::FILL, Length::px(top))
            .any(),
    };
    let ringed = frame.ringed();
    let mut face = container(avatar_with_status(
        look.face.as_ref(),
        &look.name,
        frame.face,
        look.presence.status,
        backdrop,
    ))
    .padding(Inset::all(frame.ring))
    .background(backdrop)
    .radius(ringed / 2.0)
    .focusless();
    if let Some(on_open) = on_open {
        face = face.on_press(on_open);
    }
    let mut layers = vec![
        banner,
        face.position(frame.inset - frame.ring, top - ringed / 2.0)
            .any(),
    ];
    if !actions.is_empty() {
        layers.push(
            row_of(actions)
                .gap(Space::TIGHT)
                .position(0.0, Space::SNUG.px())
                .padding(Inset::xy(Space::SNUG, Space::NONE))
                .justify(Justify::End)
                .width(frame.width)
                .any(),
        );
    }
    if let Some(mood) = look.presence.mood.as_ref().filter(|mood| !mood.is_empty()) {
        let left = frame.inset - frame.ring + ringed + Space::SNUG.px();
        layers.push(
            container(text(mood.clone()).caption().no_wrap())
                .padding(Inset::xy(Space::NORMAL, Space::TIGHT))
                .background(palette().surface_variant)
                .border(1.0, palette().outline)
                .radius(Corner::Control)
                .max_width(frame.width - left - frame.inset)
                .position(left, top + Space::SNUG.px())
                .any(),
        );
    }
    column_of(layers)
        .size(
            Length::px(frame.width),
            Length::px(frame.height(has_banner)),
        )
        .any()
}

pub(super) fn action(symbol: MaterialSymbol) -> Container {
    container(
        icon(symbol)
            .extent(Length::px(ACTION_ICON))
            .color(Color::WHITE),
    )
    .center()
    .background(Color::BLACK.with_alpha(0.5))
    .radius(ACTION / 2.0)
    .focusless()
    .size(Length::px(ACTION), Length::px(ACTION))
}
