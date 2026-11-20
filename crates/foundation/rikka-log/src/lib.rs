use std::io::IsTerminal as _;

use tracing_subscriber::EnvFilter;

pub fn install(default_filter: &str) {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));
    let subscriber = tracing_subscriber::fmt().with_env_filter(filter);
    #[cfg(windows)]
    if let Some(file) = detached::log_file() {
        let already_installed = subscriber
            .with_writer(std::sync::Mutex::new(file))
            .with_ansi(false)
            .try_init();
        drop(already_installed);
        return;
    }
    let already_installed = subscriber
        .with_writer(std::io::stderr)
        .with_ansi(std::io::stderr().is_terminal())
        .try_init();
    drop(already_installed);
}

#[cfg(windows)]
mod detached {
    use std::fs::File;
    use std::os::windows::io::AsRawHandle as _;

    const FOLDER: &str = "logs";

    pub(crate) fn log_file() -> Option<File> {
        if !std::io::stderr().as_raw_handle().is_null() {
            return None;
        }
        let program = std::env::current_exe().ok()?;
        let name = program.file_stem()?;
        let folder = rikka_dirs::Paths::resolve().ok()?.state_dir().join(FOLDER);
        rikka_dirs::ensure(&folder).ok()?;
        File::create(folder.join(name).with_extension("log")).ok()
    }
}
