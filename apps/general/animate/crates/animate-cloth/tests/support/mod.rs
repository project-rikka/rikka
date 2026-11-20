use std::sync::Arc;

use rikka_mmd::PmxImporter;
use rikka_model_core::{Import, Model, Registry};

const MIKU: &[u8] =
    include_bytes!("../../../../../../../crates/model/rikka-mmd/assets/mmd/miku.pmx");

pub(crate) fn miku() -> Model {
    Import::named_bytes(
        "miku.pmx",
        MIKU.to_vec(),
        Arc::new(Registry::new().with_importer(PmxImporter)),
    )
    .run()
    .expect("the sample model reads")
}
