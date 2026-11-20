use std::sync::{Arc, LazyLock};

use animate_body::{Body, Faces};
use animate_motion::Target;
use rikka_humanoid::Anatomy;
use rikka_mmd::{PmxImporter, Vmd, VmdImporter};
use rikka_model_core::{Import, Model, Registry};

const MIKU: &[u8] =
    include_bytes!("../../../../../../../crates/model/rikka-mmd/assets/mmd/miku.pmx");
const DANCE: &[u8] =
    include_bytes!("../../../../../../../crates/model/rikka-mmd/assets/mmd/motion.vmd");

pub(crate) static MODEL: LazyLock<Arc<Model>> = LazyLock::new(|| {
    Arc::new(
        Import::named_bytes(
            "miku.pmx",
            MIKU.to_vec(),
            Arc::new(Registry::new().with_importer(PmxImporter)),
        )
        .run()
        .expect("the sample model reads"),
    )
});

pub(crate) static BODY: LazyLock<Arc<Body>> = LazyLock::new(|| {
    Arc::new(Body::from_anatomy(&MODEL, &Anatomy::recognize(&MODEL)).expect("miku is human"))
});

pub(crate) static FACES: LazyLock<Faces> = LazyLock::new(|| Faces::from_model(&MODEL));

pub(crate) fn miku(human: bool) -> Target<'static> {
    Target {
        model: &MODEL,
        body: human.then_some(&**BODY),
        faces: &FACES,
    }
}

pub(crate) fn dance() -> Vmd {
    let mut model = Import::named_bytes(
        "motion.vmd",
        DANCE.to_vec(),
        Arc::new(Registry::new().with_importer(VmdImporter)),
    )
    .run()
    .expect("the sample motion reads");
    model.extensions.remove::<Vmd>().expect("a VMD motion")
}
