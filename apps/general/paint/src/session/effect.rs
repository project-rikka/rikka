use rikka_app::prelude::*;
use rikka_effects::{Effect, Family};
use rikka_effects_forms::Tuning;

use super::{Context, Dialog, Outcome, Session};
use crate::paint::Paint;

#[derive(Clone, Debug)]
pub(crate) struct EffectDialog {
    effect: Effect,
}

impl EffectDialog {
    pub(crate) fn session(kind: rikka_effects::Kind, context: &Context<'_>) -> Session {
        EffectDialog::reopened(kind.default_effect(), context)
    }

    pub(crate) fn reopened(effect: Effect, context: &Context<'_>) -> Session {
        let kind = effect.kind();
        Session::holding(
            EffectDialog { effect },
            move |_| EffectDialog {
                effect: kind.default_effect(),
            },
            context,
        )
    }

    pub(crate) fn effect(&self) -> &Effect {
        &self.effect
    }

    pub(crate) fn set(&mut self, effect: Effect) {
        self.effect = effect;
    }
}

impl Dialog for EffectDialog {
    fn title(&self) -> Label {
        self.effect.kind().name()
    }

    fn form(&self, context: &Context<'_>) -> Element {
        let tuning = Tuning::new(Paint::tune_effect, Paint::tune_effect)
            .submit(Paint::apply_session)
            .histogram(context.histogram.cloned());
        rikka_effects_forms::form(&self.effect, &tuning)
    }

    fn outcome(&self, _context: &Context<'_>) -> Option<Outcome> {
        let recipe = self.effect.recipe();
        (!recipe.is_empty()).then_some(Outcome::Develop(recipe))
    }

    fn develops(&self) -> bool {
        true
    }

    fn wants_histogram(&self) -> bool {
        self.effect.kind().wants_histogram()
    }

    fn repeatable_effect(&self) -> Option<Effect> {
        (self.effect.kind().family() != Family::Adjust).then(|| self.effect.clone())
    }
}
