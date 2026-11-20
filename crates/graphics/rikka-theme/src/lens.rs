use rikka_types::Lens;
use serde::{Deserialize, Deserializer};

#[derive(Deserialize)]
struct LensTable {
    bevel: Option<f32>,
    depth: Option<f32>,
    bulge: Option<f32>,
    frost: Option<f32>,
    index: Option<f32>,
    dispersion: Option<f32>,
    adapt: Option<f32>,
}

pub(crate) fn table<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Lens>, D::Error> {
    let table = LensTable::deserialize(deserializer)?;
    let setters = [
        (table.bevel, Lens::with_bevel as fn(Lens, f32) -> Lens),
        (table.depth, Lens::with_depth),
        (table.bulge, Lens::with_bulge),
        (table.frost, Lens::with_frost),
        (table.index, Lens::with_ior),
        (table.dispersion, Lens::with_dispersion),
        (table.adapt, Lens::with_adapt),
    ];
    let lens = setters
        .into_iter()
        .fold(Lens::new(), |lens, (value, set)| match value {
            Some(value) => set(lens, value),
            None => lens,
        });
    Ok(Some(lens))
}
