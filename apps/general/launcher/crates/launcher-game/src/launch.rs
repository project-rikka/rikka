use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use launcher_install::Installation;
use launcher_meta::{Environment, Feature, Platform, Substitutions, expand};

use crate::error::{Error, Result};

const LAUNCHER: &str = "rikka-launcher";
const CLIENT_ID: &str = "c4502edb-87c6-40cb-b595-64a280cf8906";

#[derive(Clone)]
pub struct Player {
    pub name: String,
    pub id: String,
    pub xuid: String,
    pub access_token: String,
}

impl std::fmt::Debug for Player {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Player")
            .field("name", &self.name)
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum QuickPlay {
    World(String),
    Server(String),
}

#[derive(Clone, Debug)]
#[must_use]
pub struct Launch {
    installation: Installation,
    game_directory: PathBuf,
    player: Player,
    memory_mb: u32,
    jvm_arguments: Vec<String>,
    window: Option<(u32, u32)>,
    fullscreen: bool,
    quick_play: Option<QuickPlay>,
    launcher_version: String,
}

fn text(path: &Path) -> String {
    path.display().to_string()
}

impl Launch {
    pub fn new(
        installation: Installation,
        game_directory: impl Into<PathBuf>,
        player: Player,
    ) -> Launch {
        Launch {
            installation,
            game_directory: game_directory.into(),
            player,
            memory_mb: 4096,
            jvm_arguments: Vec::new(),
            window: None,
            fullscreen: false,
            quick_play: None,
            launcher_version: String::new(),
        }
    }

    pub fn memory(mut self, megabytes: u32) -> Launch {
        self.memory_mb = megabytes;
        self
    }

    pub fn jvm_arguments(mut self, arguments: impl IntoIterator<Item = String>) -> Launch {
        self.jvm_arguments = arguments.into_iter().collect();
        self
    }

    pub fn window(mut self, width: u32, height: u32) -> Launch {
        self.window = Some((width, height));
        self
    }

    pub fn fullscreen(mut self, fullscreen: bool) -> Launch {
        self.fullscreen = fullscreen;
        self
    }

    pub fn quick_play(mut self, quick_play: Option<QuickPlay>) -> Launch {
        self.quick_play = quick_play;
        self
    }

    pub fn launcher_version(mut self, version: impl Into<String>) -> Launch {
        self.launcher_version = version.into();
        self
    }

    pub fn arguments(&self, platform: Platform) -> Result<Vec<String>> {
        let installation = &self.installation;
        let version = &installation.version;
        let mut environment = Environment::new(platform);
        let mut values = Substitutions::new()
            .with("auth_player_name", self.player.name.as_str())
            .with("auth_uuid", self.player.id.as_str())
            .with("auth_access_token", self.player.access_token.as_str())
            .with("auth_session", self.player.access_token.as_str())
            .with("auth_xuid", self.player.xuid.as_str())
            .with("clientid", CLIENT_ID)
            .with("user_type", "msa")
            .with("user_properties", "{}")
            .with("version_name", version.id.as_str())
            .with("version_type", version.kind.as_str())
            .with("game_directory", text(&self.game_directory))
            .with("assets_root", text(&installation.assets))
            .with("game_assets", text(&installation.assets))
            .with("assets_index_name", version.asset_index.id.as_str())
            .with("natives_directory", text(&installation.natives))
            .with("library_directory", text(&installation.libraries))
            .with("classpath_separator", platform.os.classpath_separator())
            .with("launcher_name", LAUNCHER)
            .with("launcher_version", self.launcher_version.as_str());
        let classpath: Vec<String> = installation
            .classpath
            .iter()
            .map(|path| text(path))
            .collect();
        values.set(
            "classpath",
            classpath.join(platform.os.classpath_separator()),
        );
        if let Some((width, height)) = self.window {
            environment = environment.with(Feature::CustomResolution);
            values.set("resolution_width", width.to_string());
            values.set("resolution_height", height.to_string());
        }
        match &self.quick_play {
            Some(QuickPlay::World(world)) => {
                environment = environment.with(Feature::QuickPlaySingleplayer);
                values.set("quickPlaySingleplayer", world.as_str());
            }
            Some(QuickPlay::Server(address)) => {
                environment = environment.with(Feature::QuickPlayMultiplayer);
                values.set("quickPlayMultiplayer", address.as_str());
            }
            None => {}
        }
        let mut arguments = expand(&version.arguments.jvm, &environment, &values)?;
        arguments.push(format!("-Xmx{}M", self.memory_mb));
        arguments.extend(self.jvm_arguments.iter().cloned());
        arguments.push(version.main_class.clone());
        arguments.extend(expand(&version.arguments.game, &environment, &values)?);
        if self.fullscreen {
            arguments.push("--fullscreen".to_owned());
        }
        Ok(arguments)
    }

    pub fn command(&self, platform: Platform) -> Result<Command> {
        let mut command = Command::new(&self.installation.java.executable);
        command
            .args(self.arguments(platform)?)
            .current_dir(&self.game_directory);
        if let Some(libraries) = &self.installation.java.library_path {
            let mut paths: Vec<PathBuf> = vec![libraries.clone()];
            if let Some(existing) = std::env::var_os("LD_LIBRARY_PATH") {
                paths.extend(std::env::split_paths(&existing));
            }
            let joined: OsString = std::env::join_paths(paths)
                .map_err(|error| Error::Spawn(std::io::Error::other(error)))?;
            command.env("LD_LIBRARY_PATH", joined);
        }
        Ok(command)
    }
}
