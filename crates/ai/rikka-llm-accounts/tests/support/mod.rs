use rikka_sync::{Mutex, MutexGuard};
use std::sync::OnceLock;

use rikka_llm_accounts::{Backend, Llm};
use rikka_test_keyring::TestKeyring;
use tempfile::TempDir;

static SHARED: OnceLock<TempDir> = OnceLock::new();
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

pub(crate) struct Desk {
    _turn: MutexGuard<'static, ()>,
}

impl Desk {
    pub(crate) fn fresh() -> Desk {
        let turn = ONE_AT_A_TIME.lock();
        let home = SHARED
            .get_or_init(|| TempDir::new().expect("temp home should be created"))
            .path();
        let _ = std::fs::remove_dir_all(home);
        std::fs::create_dir_all(home).expect("home should be created");
        for (variable, folder) in [
            ("XDG_CONFIG_HOME", "config"),
            ("XDG_DATA_HOME", "data"),
            ("XDG_STATE_HOME", "state"),
            ("XDG_CACHE_HOME", "cache"),
            ("XDG_CONFIG_DIRS", "system"),
            ("XDG_RUNTIME_DIR", "runtime"),
        ] {
            unsafe { std::env::set_var(variable, home.join(folder)) };
        }
        Desk { _turn: turn }
    }

    pub(crate) fn settings_text(&self) -> String {
        rikka_settings::flush_pending_writes();
        std::fs::read_to_string(settings_file()).expect("the settings were written")
    }

    pub(crate) fn write_settings(&self, text: &str) {
        rikka_settings::flush_pending_writes();
        let file = settings_file();
        if let Some(folder) = file.parent() {
            std::fs::create_dir_all(folder).expect("the settings folder should be created");
        }
        std::fs::write(file, text).expect("the settings should be written");
    }

    pub(crate) fn load(&self) -> Option<Llm> {
        TestKeyring::unlocked()?;
        rikka_settings::flush_pending_writes();
        let llm = Llm::load();
        assert_eq!(llm.unavailable(), None, "the settings could be opened");
        Some(llm)
    }
}

fn settings_file() -> std::path::PathBuf {
    SHARED
        .get()
        .expect("the desk is set up")
        .path()
        .join("config/rikka/rikka.llm.toml")
}

pub(crate) fn backend(id: &str) -> Backend {
    Backend::from_id(id).expect("backend should be registered")
}
