use std::sync::Arc;

use rikka_humanoid::Anatomy;
use rikka_mmd::{PmxImporter, Vmd, VmdImporter};
use rikka_model_core::{Import, Instance, Model, Registry};
use rikka_physics_tailor::{Budget, ClothAsset, ClothSettings};

const MIKU: &[u8] = include_bytes!("../../../../model/rikka-mmd/assets/mmd/miku.pmx");
const MOTION: &[u8] = include_bytes!("../../../../model/rikka-mmd/assets/mmd/motion.vmd");

pub(crate) fn miku() -> Arc<Model> {
    let registry = Arc::new(
        Registry::new()
            .with_importer(PmxImporter)
            .with_importer(VmdImporter),
    );
    let mut model = Import::named_bytes("miku.pmx", MIKU.to_vec(), Arc::clone(&registry))
        .run()
        .expect("the sample model");
    let motion = Import::named_bytes("motion.vmd", MOTION.to_vec(), registry)
        .run()
        .expect("the sample motion");
    let animation = motion
        .extensions
        .get::<Vmd>()
        .expect("a motion")
        .animation(&model);
    model.animations.push(animation);
    Arc::new(model)
}

pub(crate) fn asset(model: &Model) -> Arc<ClothAsset> {
    Arc::new(
        ClothAsset::build(
            model,
            &Anatomy::recognize(model),
            &ClothSettings::new(),
            Budget::default(),
        )
        .expect("cloth asset"),
    )
}

pub(crate) fn dance(model: &Arc<Model>) -> Instance {
    let mut instance = Instance::new(Arc::clone(model));
    instance.play(
        model
            .animation_id(model.animations.len() - 1)
            .expect("the dance"),
    );
    instance
}
