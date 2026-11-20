use std::sync::{Arc, LazyLock};

use animate_body::Body;
use animate_fit::Physique;
use rikka_humanoid::Anatomy;
use rikka_mmd::PmxImporter;
use rikka_model_core::{Import, Model, Registry};
use rikka_physics_tailor::{ClothAnalysis, ClothSettings, Wearer};

const MIKU: &[u8] =
    include_bytes!("../../../../../../../crates/model/rikka-mmd/assets/mmd/miku.pmx");

pub(crate) struct Sample {
    pub(crate) body: Body,
    pub(crate) wearer: Wearer,
    pub(crate) physique: Physique,
    pub(crate) bare: Physique,
}

pub(crate) static SAMPLE: LazyLock<Sample> = LazyLock::new(|| {
    let model: Model = Import::named_bytes(
        "miku.pmx",
        MIKU.to_vec(),
        Arc::new(Registry::new().with_importer(PmxImporter)),
    )
    .run()
    .expect("the sample model reads");
    let anatomy = Anatomy::recognize(&model);
    let body = Body::from_anatomy(&model, &anatomy).expect("miku is human");
    let analysis =
        ClothAnalysis::new(&model, &anatomy, &ClothSettings::new()).expect("miku analyses");
    let wearer = Wearer::from_model(&model, &anatomy, &analysis).expect("miku has a skin");
    let undressed = Wearer {
        body: wearer.body.clone(),
        garments: Vec::new(),
    };
    Sample {
        physique: Physique::new(&body, &model, &wearer),
        bare: Physique::new(&body, &model, &undressed),
        wearer,
        body,
    }
});
