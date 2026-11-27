use std::path::Path;

use rikka_model::{Import, registry};
use rikka_types::CancelToken;

use crate::details::{Clip, Details, ModelDetails};

use super::fault::{Fault, Probed, clean};

const LARGEST: u64 = 1 << 30;

pub(super) fn read(path: &Path, cancel: &CancelToken) -> Probed<Details> {
    if std::fs::metadata(path)?.len() > LARGEST {
        return Err(Fault::TooLarge);
    }
    let model = Import::open(path, registry())
        .cancel(cancel.child())
        .run()
        .map_err(|error| match cancel.is_cancelled() {
            true => Fault::Cancelled,
            false => Fault::Import(error.to_string()),
        })?;
    let clips = model
        .animations
        .iter()
        .enumerate()
        .map(|(index, animation)| Clip {
            name: clean(&animation.name).unwrap_or_else(|| format!("{}", index + 1)),
            seconds: animation.duration(),
        })
        .collect();
    Ok(Details::Model(Box::new(ModelDetails {
        vertices: Some(
            model
                .meshes
                .iter()
                .map(|mesh| mesh.vertices.len() as u64)
                .sum(),
        ),
        materials: u32::try_from(model.materials.len()).ok(),
        meshes: u32::try_from(model.meshes.len()).ok(),
        clips,
        ..ModelDetails::default()
    })))
}
