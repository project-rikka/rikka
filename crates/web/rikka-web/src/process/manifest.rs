use std::path::Path;

use serde::Serialize;

use crate::error::{Error, Result};

pub(crate) const HOST_NAME: &str = "os.rikka.web";

const FOLDER: &str = "NativeMessagingHosts";

#[derive(Serialize)]
struct Manifest<'a> {
    name: &'a str,
    description: &'a str,
    path: &'a Path,
    #[serde(rename = "type")]
    kind: &'a str,
    allowed_origins: [String; 1],
}

pub(crate) fn install(profile: &Path, host: &Path, extension_id: &str) -> Result<()> {
    let folder = profile.join(FOLDER);
    let failed = |source| Error::Profile {
        path: folder.clone(),
        source,
    };
    std::fs::create_dir_all(&folder).map_err(failed)?;
    let manifest = Manifest {
        name: HOST_NAME,
        description: "Rikka",
        path: host,
        kind: "stdio",
        allowed_origins: [format!("chrome-extension://{extension_id}/")],
    };
    let encoded = serde_json::to_vec_pretty(&manifest).expect("the manifest always encodes");
    std::fs::write(folder.join(format!("{HOST_NAME}.json")), encoded).map_err(failed)
}
