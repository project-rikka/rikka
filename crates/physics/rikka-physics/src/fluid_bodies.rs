use rikka_model_core::Model;
use rikka_physics_fluid::{BodyId, Error as FluidError, Result as FluidResult};

use crate::error::Result;
use crate::layout::Layout;

pub(crate) fn add_bodies(
    layout: &Layout,
    mut add: impl FnMut(&Model) -> FluidResult<BodyId>,
) -> Result<Vec<(usize, BodyId)>> {
    let mut bodies = Vec::with_capacity(layout.figures.len());
    for (index, figure) in layout.figures.iter().enumerate() {
        match add(&figure.model) {
            Ok(body) => bodies.push((index, body)),
            Err(FluidError::NotABody(_)) => {}
            Err(failure) => return Err(failure.into()),
        }
    }
    Ok(bodies)
}

pub(crate) fn body_of(bodies: &[(usize, BodyId)], figure: usize) -> Option<BodyId> {
    bodies
        .iter()
        .find(|(index, _)| *index == figure)
        .map(|(_, body)| *body)
}
