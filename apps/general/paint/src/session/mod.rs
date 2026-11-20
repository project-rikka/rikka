mod canvas_size;
mod convert;
mod effect;
pub(crate) mod form;
mod kind;
mod last_effect;
mod layer_properties;
mod modify;
mod panel;
mod resize;
mod rotate;
mod save_options;

use std::any::Any;
use std::path::PathBuf;
use std::rc::Rc;

use paint_document::{Edit, Layer, LayerId, LayerKind, Region, Saving};
use rikka_app::prelude::*;
use rikka_effects::Effect;
use rikka_image_edit::{Histogram, Recipe, Step};

use crate::paint::Paint;
use crate::tab::DocumentInfo;

pub(crate) use effect::EffectDialog;
pub(crate) use kind::Kind;
pub(crate) use last_effect::LastEffect;
pub(crate) use modify::Modification;
pub(crate) use panel::panel;
pub(crate) use save_options::{SaveChoices, SaveOptions};

pub(crate) trait Dialog: Any + std::fmt::Debug {
    fn title(&self) -> Label;

    fn form(&self, context: &Context<'_>) -> Element;

    fn outcome(&self, context: &Context<'_>) -> Option<Outcome>;

    fn develops(&self) -> bool {
        false
    }

    fn confirm(&self) -> Label {
        tr!("paint.session.apply")
    }

    fn previews(&self) -> bool {
        self.develops()
    }

    fn wants_histogram(&self) -> bool {
        false
    }

    fn refusal(&self) -> Option<Label> {
        None
    }

    fn repeatable_effect(&self) -> Option<Effect> {
        None
    }
}

#[derive(Debug)]
pub(crate) enum Outcome {
    Develop(Recipe),
    Edit(Edit),
    Save { path: PathBuf, saving: Saving },
}

#[derive(Debug)]
pub(crate) struct Context<'a> {
    pub(crate) facts: &'a DocumentInfo,
    pub(crate) layer: Option<&'a Layer>,
    pub(crate) selection: Rc<Region>,
    pub(crate) histogram: Option<&'a Histogram>,
}

impl Context<'_> {
    pub(crate) fn develop_refusal(&self) -> Option<Label> {
        let Some(layer) = self.layer else {
            return Some(tr!("paint.session.refusal.no_layer"));
        };
        if layer.kind != LayerKind::Pixel {
            return Some(tr!("paint.session.refusal.not_pixels"));
        }
        (!layer.visible).then(|| tr!("paint.session.refusal.hidden"))
    }

    pub(crate) fn masked(&self, recipe: &Recipe) -> paint_document::Result<Recipe> {
        let canvas = self.facts.canvas();
        let selection = self.selection.clipped(canvas);
        if selection.is_empty() || selection.encloses(canvas) {
            return Ok(recipe.clone());
        }
        let mask = selection.mask(canvas.width, canvas.height)?;
        Ok(recipe
            .steps()
            .iter()
            .fold(Recipe::new(), |masked, step: &Step| {
                masked.then(step.clone().masked(mask.clone()))
            }))
    }
}

type DialogFactory = Rc<dyn Fn(&Context<'_>) -> Box<dyn Dialog>>;

pub(crate) struct Session {
    dialog: Box<dyn Dialog>,
    factory: DialogFactory,
    layer: Option<LayerId>,
    previewing: bool,
    resets: u32,
}

impl Session {
    pub(crate) fn open<D: Dialog>(
        factory: fn(&Context<'_>) -> D,
        context: &Context<'_>,
    ) -> Session {
        Session::holding(factory(context), factory, context)
    }

    pub(crate) fn holding<D: Dialog>(
        dialog: D,
        factory: impl Fn(&Context<'_>) -> D + 'static,
        context: &Context<'_>,
    ) -> Session {
        Session {
            dialog: Box::new(dialog),
            factory: Rc::new(move |context| Box::new(factory(context))),
            layer: context.layer.map(|layer| layer.id),
            previewing: true,
            resets: 0,
        }
    }

    pub(crate) fn dialog(&self) -> &dyn Dialog {
        &*self.dialog
    }

    pub(crate) fn layer(&self) -> Option<LayerId> {
        self.layer
    }

    pub(crate) fn previews(&self) -> bool {
        self.dialog.previews()
    }

    pub(crate) fn is_previewing(&self) -> bool {
        self.previewing && self.dialog.previews()
    }

    pub(crate) fn set_previewing(&mut self, previewing: bool) {
        self.previewing = previewing;
    }

    pub(crate) fn reset(&mut self, context: &Context<'_>) {
        self.dialog = (self.factory)(context);
        self.resets += 1;
    }

    pub(crate) fn resets(&self) -> u32 {
        self.resets
    }

    pub(crate) fn refusal(&self, context: &Context<'_>) -> Option<Label> {
        match self.dialog.develops() {
            true => context.develop_refusal().or_else(|| self.dialog.refusal()),
            false => self.dialog.refusal(),
        }
    }

    pub(crate) fn last_effect(&self, context: &Context<'_>) -> Option<LastEffect> {
        let effect = self.dialog.repeatable_effect()?;
        if self.refusal(context).is_some() {
            return None;
        }
        let Some(Outcome::Develop(recipe)) = self.dialog.outcome(context) else {
            return None;
        };
        Some(LastEffect::new(self.dialog.title(), recipe, Some(effect)))
    }

    pub(crate) fn dialog_as<D: Dialog>(&self) -> Option<&D> {
        let dialog: &dyn Any = &*self.dialog;
        dialog.downcast_ref()
    }

    pub(crate) fn tune<D: Dialog>(&mut self, change: impl FnOnce(&mut D)) -> bool {
        let dialog: &mut dyn Any = &mut *self.dialog;
        match dialog.downcast_mut::<D>() {
            Some(dialog) => {
                change(dialog);
                true
            }
            None => false,
        }
    }
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("dialog", &self.dialog)
            .field("layer", &self.layer)
            .field("previewing", &self.previewing)
            .field("resets", &self.resets)
            .finish_non_exhaustive()
    }
}

pub(crate) fn tune<D: Dialog, A: 'static>(
    change: impl Fn(&mut D, A) + Clone + 'static,
) -> impl Fn(&mut Paint, A) + Clone + 'static {
    move |paint: &mut Paint, value: A| paint.tune(|dialog: &mut D| change(dialog, value))
}

pub(crate) fn tap<D: Dialog>(
    change: impl Fn(&mut D) + Clone + 'static,
) -> impl Fn(&mut Paint) + Clone + 'static {
    move |paint: &mut Paint| paint.tune(|dialog: &mut D| change(dialog))
}
