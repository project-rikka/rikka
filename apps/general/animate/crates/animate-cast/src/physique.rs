use std::sync::Arc;

use animate_body::Body;
use animate_fit::Physique;
use rikka_model::Model;
use rikka_model::humanoid::Anatomy;
use rikka_physics_tailor::{ClothAnalysis, Wearer};

pub type BodyFit = Result<Option<Arc<Physique>>, BodyFitError>;

#[derive(Debug, thiserror::Error)]
#[error("cannot skin the body for fitting")]
pub struct BodyFitError(#[source] rikka_physics_tailor::Error);

pub fn fit_physique(
    body: Option<&Body>,
    model: &Model,
    anatomy: &Anatomy,
    analysis: Option<&ClothAnalysis>,
) -> BodyFit {
    let Some((body, analysis)) = body.zip(analysis) else {
        return Ok(None);
    };
    let wearer = Wearer::from_model(model, anatomy, analysis).map_err(BodyFitError)?;
    Ok(Some(Arc::new(Physique::new(body, model, &wearer))))
}
