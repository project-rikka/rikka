use rikka_sync::{Mutex, MutexGuard};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub(crate) const LOOKUP_JSON: &[u8] = include_bytes!("../lookup.json");

const SCRIPT: &str = r#"#!/bin/sh
printf '%s\n' "$@" > "FOLDER/args"
cat "FOLDER/stdout"
cat "FOLDER/stderr" >&2
exit "$(cat "FOLDER/status")"
"#;

pub(crate) struct Nix {
    folder: &'static Path,
    _guard: MutexGuard<'static, ()>,
}

impl Nix {
    pub(crate) fn with_stdout(stdout: &[u8]) -> Nix {
        Nix::install(stdout, b"", 0)
    }

    pub(crate) fn failing(stderr: &[u8]) -> Nix {
        Nix::install(b"", stderr, 1)
    }

    fn install(stdout: &[u8], stderr: &[u8], status: u8) -> Nix {
        static LOCK: Mutex<()> = Mutex::new(());
        let guard = LOCK.lock();
        let folder = fake_nix();
        std::fs::write(folder.join("stdout"), stdout).expect("failed to write stdout");
        std::fs::write(folder.join("stderr"), stderr).expect("failed to write stderr");
        std::fs::write(folder.join("status"), status.to_string()).expect("failed to write status");
        Nix {
            folder,
            _guard: guard,
        }
    }

    pub(crate) fn arguments(&self) -> String {
        std::fs::read_to_string(self.folder.join("args")).expect("nix was not run")
    }
}

fn fake_nix() -> &'static Path {
    static FOLDER: OnceLock<PathBuf> = OnceLock::new();
    FOLDER.get_or_init(|| {
        let folder =
            std::env::temp_dir().join(format!("nixpkgs-search-nix-{}", std::process::id()));
        std::fs::create_dir_all(&folder).expect("failed to create fake nix folder");
        let nix = folder.join("nix");
        std::fs::write(
            &nix,
            SCRIPT.replace("FOLDER", &folder.display().to_string()),
        )
        .expect("failed to write fake nix");
        std::fs::set_permissions(&nix, std::fs::Permissions::from_mode(0o755))
            .expect("failed to make fake nix executable");
        let path = std::env::var_os("PATH").unwrap_or_default();
        let mut searched = vec![folder.clone()];
        searched.extend(std::env::split_paths(&path));
        let joined = std::env::join_paths(searched).expect("invalid PATH");
        unsafe { std::env::set_var("PATH", joined) };
        folder
    })
}
