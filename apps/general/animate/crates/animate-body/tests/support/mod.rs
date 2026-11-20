use std::sync::Arc;

use rikka_mmd::{PmxImporter, VmdImporter};
use rikka_model_core::{Import, Model, Registry};

const MIKU: &[u8] =
    include_bytes!("../../../../../../../crates/model/rikka-mmd/assets/mmd/miku.pmx");

pub(crate) fn read(bytes: &[u8], name: &str) -> Model {
    Import::named_bytes(
        name,
        bytes.to_vec(),
        Arc::new(
            Registry::new()
                .with_importer(PmxImporter)
                .with_importer(VmdImporter),
        ),
    )
    .run()
    .expect("the sample reads")
}

pub(crate) fn miku() -> Model {
    read(MIKU, "miku.pmx")
}
