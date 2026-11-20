use std::sync::{Arc, OnceLock};

use animate_body::Body;
use rikka_humanoid::Anatomy;
use rikka_mmd::PmxImporter;
use rikka_model_core::{Import, Instance, Model, Registry};
use rikka_physics_tailor::{Budget, ClothAsset, ClothSettings};

const MIKU: &[u8] =
    include_bytes!("../../../../../../../crates/model/rikka-mmd/assets/mmd/miku.pmx");

pub(crate) struct Sample {
    pub(crate) model: Arc<Model>,
    pub(crate) body: Body,
    pub(crate) cloth_asset: Arc<ClothAsset>,
}

pub(crate) fn miku() -> &'static Sample {
    static SAMPLE: OnceLock<Sample> = OnceLock::new();
    SAMPLE.get_or_init(|| {
        let model = Import::named_bytes(
            "miku.pmx",
            MIKU.to_vec(),
            Arc::new(Registry::new().with_importer(PmxImporter)),
        )
        .run()
        .expect("the sample reads");
        let anatomy = Anatomy::recognize(&model);
        let body = Body::from_anatomy(&model, &anatomy).expect("the sample is humanoid");
        let cloth_asset =
            ClothAsset::build(&model, &anatomy, &ClothSettings::new(), Budget::default())
                .expect("the sample is figure_cloth");
        Sample {
            model: Arc::new(model),
            body,
            cloth_asset: Arc::new(cloth_asset),
        }
    })
}

pub(crate) fn standing(sample: &Sample) -> Instance {
    let mut instance = Instance::new(Arc::clone(&sample.model));
    instance.refresh();
    instance
}
