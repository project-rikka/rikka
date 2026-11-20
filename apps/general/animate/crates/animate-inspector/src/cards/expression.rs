use animate_body::{Expression, Faces, MorphWeights};
use animate_ui::{choice, labelled};
use rikka_app::prelude::*;
use rikka_widgets::column;

use crate::{Reach, wording};

pub(super) fn expression(
    reach: Reach,
    faces: &Faces,
    weights: &MorphWeights,
    remembered: Option<Expression>,
) -> impl View {
    let available = |expression: &Expression| faces.has(*expression);
    let weight = |expression: Expression| faces.strength(weights, expression);
    let mood = Expression::MOODS
        .into_iter()
        .filter(|mood| weight(*mood) > 0.0)
        .max_by(|one, two| weight(*one).total_cmp(&weight(*two)))
        .or(remembered.filter(available));
    let strength: f32 = mood.map_or(0.0, weight);
    column! {
        row_of(Expression::MOODS.into_iter().filter(available).map(|expression| {
            let on = mood == Some(expression);
            choice(wording::expression(expression), on).on_press(reach.act(move |studio| {
                let desired = (!on).then_some(expression);
                studio.controls_mut().mood = desired;
                studio.express(&mut |faces, weights| {
                    express_mood(faces, weights, desired, strength.max(0.5));
                });
            }))
        }))
        .gap(Space::TIGHT)
        .wrap_items()
        .fill_width();
        labelled(
            tr!("animate.face.strength"),
            slider(strength)
                .range(0.0..=1.0)
                .control_size(ControlSize::Small)
                .enabled(mood.is_some())
                .on_change(reach.act_with(move |studio, value: f32| {
                    studio.controls_mut().mood = mood;
                    studio.express(&mut |faces, weights| express_mood(faces, weights, mood, value));
                }))
                .fill_width(),
            None,
        );
    }
    .gap(Space::TIGHT)
    .fill_width()
}

fn express_mood(
    faces: &Faces,
    weights: &mut MorphWeights,
    mood: Option<Expression>,
    strength: f32,
) {
    for other_mood in Expression::MOODS {
        faces.express(weights, other_mood, 0.0);
    }
    if let Some(mood) = mood {
        faces.express(weights, mood, strength);
    }
}
