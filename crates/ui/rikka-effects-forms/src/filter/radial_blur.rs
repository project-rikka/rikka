use crate::editor::Pad;
use crate::form::Form;
use crate::rows;
use crate::tuner::Tuner;
use rikka_effects::filter::radial_blur::{
    AMOUNT, CENTER_X, CENTER_Y, Grade, Method, RadialBlur, SPIN,
};
use rikka_ui::{Element, Space, Styled, TextStyled, View, text, tr};
use rikka_widgets::row;

const GRADES: [Grade; 3] = [Grade::Draft, Grade::Good, Grade::Best];

impl Form for RadialBlur {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        let effect = tuner.current();
        let amount = match effect.method {
            Method::Spin => tuner.amount(SPIN),
            Method::Zoom => tuner.amount(AMOUNT),
        };
        let quality = GRADES
            .iter()
            .position(|grade| *grade == effect.quality)
            .unwrap_or(1);
        let (moving, moved) = tuner.staged(|effect: &mut RadialBlur, (x, y): (f32, f32)| {
            effect.center_x = x;
            effect.center_y = y;
        });
        rows::form_sheet(vec![
            rows::heading(tr!("effects.radial_blur.method")),
            tuner.segments(
                usize::from(effect.method == Method::Zoom),
                vec![
                    tr!("effects.radial_blur.spin"),
                    tr!("effects.radial_blur.zoom"),
                ],
                |effect: &mut RadialBlur, at: usize| {
                    effect.method = match at {
                        1 => Method::Zoom,
                        _ => Method::Spin,
                    };
                },
            ),
            amount,
            rows::heading(tr!("effects.radial_blur.quality")),
            tuner.segments(
                quality,
                vec![
                    tr!("effects.radial_blur.draft"),
                    tr!("effects.radial_blur.good"),
                    tr!("effects.radial_blur.best"),
                ],
                |effect: &mut RadialBlur, at: usize| effect.quality = GRADES[at],
            ),
            rows::heading(tr!("effects.radial_blur.center")),
            row! {
                Pad::new((effect.center_x, effect.center_y), moving, moved);
                text(tr!(
                    "effects.radial_blur.center_at",
                    x = (effect.center_x * 100.0).round(),
                    y = (effect.center_y * 100.0).round()
                ))
                .caption()
                .muted()
                .grow(1.0)
                .min_width(0.0);
                tuner.adornment(CENTER_X);
                tuner.adornment(CENTER_Y);
            }
            .gap(Space::NORMAL)
            .fill_width()
            .any(),
        ])
    }
}
