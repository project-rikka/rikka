use std::path::{Path, PathBuf};

use ade_scope::Scope;
use rikka_app::prelude::*;
use rikka_mcp::{Catalog, Hub, Place, Vault};

const PERSONAL: &str = "personal";
const PROJECT: &str = "project";
const DECLARED: &str = "mcp.json";

const PERSONAL_ABOUT: &str = "every project of this person, private to them";
const PROJECT_ABOUT: &str =
    "this project only; it may be committed and shared, so it must never hold secrets";

pub(crate) fn hub(root: Option<&Path>) -> Hub {
    let mut building = Hub::builder();
    if let Some(path) = personal() {
        building = building.place(Place::new(PERSONAL, path).with_description(PERSONAL_ABOUT));
    }
    if let Some(root) = root {
        building = building
            .place(Place::new(PROJECT, Catalog::path_in(root)).with_description(PROJECT_ABOUT))
            .folder(root);
    }
    building.vault(Vault::from_runtime_dir()).build()
}

pub(crate) fn titled(place: &str) -> Label {
    match place {
        PERSONAL => tr!("ade.settings.servers.personal"),
        PROJECT => tr!("ade.settings.servers.project"),
        other => Label::from(other),
    }
}

fn personal() -> Option<PathBuf> {
    Some(Scope::User.rikka(None)?.join(DECLARED))
}
