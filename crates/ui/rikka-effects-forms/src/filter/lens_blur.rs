use crate::form::Form;
use crate::rows;
use crate::tuner::Tuner;
use rikka_effects::filter::lens_blur::{
    BRIGHTNESS, CURVATURE, Iris, LensBlur, RADIUS, ROTATION, THRESHOLD,
};
use rikka_ui::{Element, tr};

const IRISES: [Iris; 7] = [
    Iris::Circle,
    Iris::Triangle,
    Iris::Square,
    Iris::Pentagon,
    Iris::Hexagon,
    Iris::Heptagon,
    Iris::Octagon,
];

impl Form for LensBlur {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        let effect = tuner.current();
        let mut rows = vec![
            rows::heading(tr!("effects.lens_blur.iris")),
            tuner.choice(
                tr!("effects.lens_blur.shape"),
                IRISES.iter().position(|iris| *iris == effect.shape),
                IRISES.map(Iris::name).to_vec(),
                |effect: &mut LensBlur, at: usize| effect.shape = IRISES[at],
            ),
            tuner.amount(RADIUS),
        ];
        if effect.shape.blades() > 0 {
            rows.extend([tuner.amount(CURVATURE), tuner.amount(ROTATION)]);
        }
        rows.extend([
            rows::heading(tr!("effects.lens_blur.specular")),
            tuner.amount(BRIGHTNESS),
            tuner.amount(THRESHOLD),
        ]);
        rows::form_sheet(rows)
    }
}
