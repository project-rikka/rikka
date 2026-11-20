use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

pub(crate) const PROGRAM: &str = "FRAME_COST_APP";

const DATA: &str = "FRAME_COST_APP_DATA";

const CONFIG: &str = "FRAME_COST_APP_CONFIG";

const STARTING: Duration = Duration::from_secs(5);

pub(crate) struct App {
    child: Child,
    log: PathBuf,
}

impl App {
    pub(crate) fn launch(socket: &Path, config: &Path) -> Self {
        let program = std::env::var_os(PROGRAM)
            .map(PathBuf::from)
            .unwrap_or_else(|| panic!("{PROGRAM} is set to the rikka-app binary to measure"));
        let home = config.join("app");
        let data = std::env::var_os(DATA).map_or_else(|| home.join("data"), PathBuf::from);
        if let Some(settings) = std::env::var_os(CONFIG) {
            for entry in std::fs::read_dir(settings).unwrap().map_while(Result::ok) {
                std::fs::copy(entry.path(), config.join("rikka").join(entry.file_name())).unwrap();
            }
        }
        let log = std::env::temp_dir().join(format!("frame-cost-app-{}.log", std::process::id()));
        let child = Command::new(&program)
            .env("WAYLAND_DISPLAY", socket)
            .env("XDG_DATA_HOME", data)
            .env("XDG_CACHE_HOME", home.join("cache"))
            .stdout(Stdio::null())
            .stderr(std::fs::File::create(&log).unwrap())
            .spawn()
            .unwrap_or_else(|error| panic!("cannot start {}: {error}", program.display()));
        std::thread::sleep(STARTING);
        Self { child, log }
    }

    pub(crate) fn log(&self) -> &Path {
        &self.log
    }
}

impl Drop for App {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
