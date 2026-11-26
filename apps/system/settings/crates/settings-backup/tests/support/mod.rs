use std::path::Path;

use rikka_backup::{Bundle, Origin};
use settings_backup::Dirs;

pub(crate) fn dirs(home: &Path) -> Dirs {
    Dirs {
        home: home.to_owned(),
        config: home.join(".config/rikka"),
        data: home.join(".local/share/rikka"),
        state: home.join(".local/state/rikka"),
    }
}

pub(crate) fn write(path: &Path, contents: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
}

pub(crate) fn bundle(home: &Path) -> Bundle {
    Bundle {
        origin: Origin {
            host: "old".to_owned(),
            user: "alice".to_owned(),
            home: home.to_str().unwrap().to_owned(),
            created: 0,
        },
        modules: Vec::new(),
        preferences: Vec::new(),
        files: Vec::new(),
        credentials: Vec::new(),
    }
}
