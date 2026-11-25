use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use launcher_nbt::{Compound, List, Tag};

use crate::error::{Error, Result};
use crate::folder::entries;

const SAVES: &str = "saves";
const LEVEL: &str = "level.dat";
const ICON: &str = "icon.png";
const SERVERS: &str = "servers.dat";
const MAX_LEVEL: u64 = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GameMode {
    Survival,
    Creative,
    Adventure,
    Spectator,
}

impl GameMode {
    fn from_id(id: i64) -> GameMode {
        match id {
            1 => GameMode::Creative,
            2 => GameMode::Adventure,
            3 => GameMode::Spectator,
            _ => GameMode::Survival,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct World {
    pub folder: String,
    pub path: PathBuf,
    pub name: String,
    pub last_played: Option<i64>,
    pub mode: GameMode,
    pub hardcore: bool,
    pub version: Option<String>,
    pub icon: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Server {
    pub name: String,
    pub address: String,
    pub icon: Option<Vec<u8>>,
}

fn read_level(path: &Path) -> Result<Compound> {
    let file = fs::File::open(path).map_err(Error::io(path))?;
    let mut bytes = Vec::new();
    GzDecoder::new(file)
        .take(MAX_LEVEL)
        .read_to_end(&mut bytes)
        .map_err(Error::io(path))?;
    let (_, root) = launcher_nbt::read(&bytes).map_err(|source| Error::Nbt {
        path: path.to_owned(),
        source,
    })?;
    Ok(root)
}

#[derive(Debug, Default)]
pub struct Worlds {
    pub worlds: Vec<World>,
    pub problems: Vec<Error>,
}

fn world(path: PathBuf, folder: &str, level: &Path) -> Result<World> {
    let root = read_level(level)?;
    let empty = Compound::new();
    let data = root.compound("Data").unwrap_or(&empty);
    let icon = path.join(ICON);
    Ok(World {
        folder: folder.to_owned(),
        name: data
            .string("LevelName")
            .filter(|name| !name.is_empty())
            .unwrap_or(folder)
            .to_owned(),
        last_played: data.integer("LastPlayed"),
        mode: GameMode::from_id(data.integer("GameType").unwrap_or(0)),
        hardcore: data.integer("hardcore").is_some_and(|value| value != 0),
        version: data
            .compound("Version")
            .and_then(|version| version.string("Name"))
            .map(str::to_owned),
        icon: icon.is_file().then_some(icon),
        path,
    })
}

pub(crate) fn worlds(root: &Path) -> Result<Worlds> {
    let mut listed = Worlds::default();
    for entry in entries(&root.join(SAVES))? {
        let path = entry.path();
        let level = path.join(LEVEL);
        let Some(folder) = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_owned)
        else {
            continue;
        };
        if !level.is_file() {
            continue;
        }
        match world(path, &folder, &level) {
            Ok(world) => listed.worlds.push(world),
            Err(problem) => listed.problems.push(problem),
        }
    }
    listed
        .worlds
        .sort_by_key(|world| std::cmp::Reverse(world.last_played.unwrap_or(0)));
    Ok(listed)
}

fn read_servers(root: &Path) -> Result<Compound> {
    let path = root.join(SERVERS);
    match fs::read(&path) {
        Ok(bytes) => launcher_nbt::read(&bytes)
            .map(|(_, root)| root)
            .map_err(|source| Error::Nbt { path, source }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Compound::new()),
        Err(source) => Err(Error::Io { path, source }),
    }
}

fn write_servers(root: &Path, servers: &Compound) -> Result<()> {
    let path = root.join(SERVERS);
    let bytes = launcher_nbt::write("", servers).map_err(|source| Error::Nbt {
        path: path.clone(),
        source,
    })?;
    rikka_fs::replace(&path, &bytes).map_err(Error::io(&path))
}

pub(crate) fn servers(root: &Path) -> Result<Vec<Server>> {
    use base64::Engine;
    let servers = read_servers(root)?;
    let Some(list) = servers.list("servers") else {
        return Ok(Vec::new());
    };
    Ok(list
        .items()
        .iter()
        .filter_map(|item| match item {
            Tag::Compound(server) => Some(Server {
                name: server.string("name").unwrap_or_default().to_owned(),
                address: server.string("ip")?.to_owned(),
                icon: server
                    .string("icon")
                    .and_then(|icon| base64::engine::general_purpose::STANDARD.decode(icon).ok()),
            }),
            _ => None,
        })
        .collect())
}

pub(crate) fn add_server(root: &Path, name: &str, address: &str) -> Result<()> {
    let mut servers = read_servers(root)?;
    let mut list = servers.list("servers").cloned().unwrap_or_else(List::empty);
    let mut server = Compound::new();
    server.insert("name", Tag::String(name.to_owned()));
    server.insert("ip", Tag::String(address.to_owned()));
    list.push(Tag::Compound(server))
        .map_err(|source| Error::Nbt {
            path: root.join(SERVERS),
            source,
        })?;
    servers.insert("servers", Tag::List(list));
    write_servers(root, &servers)
}

pub(crate) fn remove_server(root: &Path, address: &str) -> Result<()> {
    let mut servers = read_servers(root)?;
    let Some(mut list) = servers.list("servers").cloned() else {
        return Ok(());
    };
    let found = list.items().iter().position(
        |item| matches!(item, Tag::Compound(server) if server.string("ip") == Some(address)),
    );
    if let Some(index) = found {
        list.remove(index);
        servers.insert("servers", Tag::List(list));
        write_servers(root, &servers)?;
    }
    Ok(())
}
