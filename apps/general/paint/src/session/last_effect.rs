use rikka_app::prelude::*;
use rikka_effects::Effect;
use rikka_image_edit::Recipe;

use super::effect::EffectDialog;
use super::{Context, Session};

#[derive(Clone, Debug)]
pub(crate) struct LastEffect {
    name: Label,
    recipe: Recipe,
    effect: Option<Effect>,
}

impl LastEffect {
    pub(crate) fn new(name: Label, recipe: Recipe, effect: Option<Effect>) -> LastEffect {
        LastEffect {
            name,
            recipe,
            effect,
        }
    }

    pub(crate) fn name(&self) -> &Label {
        &self.name
    }

    pub(crate) fn recipe(&self) -> &Recipe {
        &self.recipe
    }

    pub(crate) fn reopen(&self, context: &Context<'_>) -> Option<Session> {
        let effect = self.effect.clone()?;
        Some(EffectDialog::reopened(effect, context))
    }
}
