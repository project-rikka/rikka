use std::path::PathBuf;

use rikka_dirs::UserDirs;
use rikka_settings::{Key, Schema};

use crate::Preferences;

const FOLDER: &str = "download-folder";
const ASK: &str = "download-ask";

impl Preferences {
    pub fn download_folder(&self) -> PathBuf {
        self.observers.read();
        let chosen: String = self.settings.borrow().get(FOLDER);
        match chosen.is_empty() {
            false => PathBuf::from(chosen),
            true => UserDirs::load().downloads(),
        }
    }

    pub fn set_download_folder(&self, folder: &std::path::Path) {
        let folder = folder.to_string_lossy().into_owned();
        self.put::<String>(FOLDER, folder);
    }

    pub fn download_asks(&self) -> bool {
        self.observers.read();
        self.settings.borrow().get(ASK)
    }

    pub fn set_download_asks(&self, asks: bool) {
        self.put::<bool>(ASK, asks);
    }
}

pub(super) fn keys(schema: Schema) -> Schema {
    schema
        .key(Key::string(FOLDER, "").summary("Folder downloads are saved to"))
        .key(Key::bool(ASK, false).summary("Ask where to save each download"))
}
