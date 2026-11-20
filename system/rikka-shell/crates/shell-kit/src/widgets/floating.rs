use rikka_types::{Key, Modifiers};
use rikka_ui::{Align, Element, Handler, IntoHandler, Length, Styled, View, is_exiting};
use rikka_widgets::{column, spacer};

use crate::widgets::grip::grip;
use crate::widgets::spot::Spot;
use crate::widgets::{entrance, scrim, sheet};

const WIDEST: f32 = 0.94;

#[derive(Debug)]
pub struct Floating {
    panel: Element,
    spot: Spot,
    on_dismiss: Handler,
    on_key: Option<Handler<(Key, Modifiers), bool>>,
    width: Option<f32>,
    down: f32,
}

#[must_use]
pub fn floating<Marker>(
    spot: &Spot,
    panel: impl View,
    on_dismiss: impl IntoHandler<(), (), Marker>,
) -> Floating {
    Floating {
        panel: panel.build(),
        spot: spot.clone(),
        on_dismiss: on_dismiss.into_handler(),
        on_key: None,
        width: None,
        down: 0.0,
    }
}

impl Floating {
    #[must_use]
    pub fn on_key<Marker>(
        mut self,
        on_key: impl IntoHandler<(Key, Modifiers), bool, Marker>,
    ) -> Self {
        self.on_key = Some(on_key.into_handler());
        self
    }

    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    #[must_use]
    pub fn down(mut self, down: f32) -> Self {
        self.down = down;
        self
    }
}

impl View for Floating {
    fn build(self) -> Element {
        let Floating {
            panel,
            spot,
            on_dismiss,
            on_key,
            width,
            down,
        } = self;

        let at = spot.at();
        let window = (!is_exiting()).then(move || {
            let on_key = on_key.unwrap_or_else(|| Handler::new(|_: Key, _: Modifiers| false));
            let mut window = sheet(grip(panel, spot), on_key)
                .keeps_tab()
                .build()
                .max_width(Length::portion(WIDEST))
                .transition(entrance());
            if let Some(width) = width {
                window = window.width(width);
            }
            match at {
                Some(at) => window.position(at.x, at.y),
                None => window,
            }
        });

        scrim(
            column! {
                spacer().height(Length::portion(down));
                window
            }
            .align_items(Align::Center)
            .fill(),
            on_dismiss,
        )
        .build()
    }
}
