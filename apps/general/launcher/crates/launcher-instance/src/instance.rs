use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::config::{Config, ContentRecord};
use crate::content::{
    self, ContentFile, ContentKind, DISABLED, Place, checked_name, record_for, scan,
};
use crate::error::{Error, Result};
use crate::files::{self, LogFile, Screenshot};
use crate::folder::entries;
use crate::worlds::{self, Server, Worlds};

pub(crate) const CONFIG: &str = "instance.json";
const ICON: &str = "icon.png";
const SAVES: &str = "saves";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instance {
    pub id: String,
    pub path: PathBuf,
    pub config: Config,
    icon: Option<PathBuf>,
}

fn found_icon(path: &Path) -> Option<PathBuf> {
    let icon = path.join(ICON);
    icon.is_file().then_some(icon)
}

impl Instance {
    pub(crate) fn new(id: String, path: PathBuf, config: Config) -> Instance {
        Instance {
            icon: found_icon(&path),
            id,
            path,
            config,
        }
    }

    pub(crate) fn load(id: String, path: PathBuf) -> Result<Instance> {
        let file = path.join(CONFIG);
        let bytes = fs::read(&file).map_err(Error::io(&file))?;
        let config = serde_json::from_slice(&bytes)
            .map_err(|source| Error::Config { path: file, source })?;
        Ok(Instance::new(id, path, config))
    }

    pub(crate) fn save(&self) -> Result<()> {
        let file = self.path.join(CONFIG);
        let bytes = serde_json::to_vec_pretty(&self.config).map_err(|source| Error::Config {
            path: file.clone(),
            source,
        })?;
        rikka_fs::replace(&file, &bytes).map_err(Error::io(&file))
    }

    #[must_use]
    pub fn icon(&self) -> Option<&Path> {
        self.icon.as_deref()
    }

    pub fn set_icon(&mut self, source: &Path) -> Result<()> {
        let bytes = fs::read(source).map_err(Error::io(source))?;
        let icon = self.path.join(ICON);
        rikka_fs::replace(&icon, &bytes).map_err(Error::io(&icon))?;
        self.icon = Some(icon);
        Ok(())
    }

    fn world_folders(&self) -> Result<Vec<String>> {
        let saves = self.path.join(SAVES);
        let mut folders = Vec::new();
        for entry in entries(&saves)? {
            if entry.file_type().map_err(Error::io(entry.path()))?.is_dir()
                && let Some(name) = entry.file_name().to_str()
            {
                folders.push(name.to_owned());
            }
        }
        folders.sort();
        Ok(folders)
    }

    pub fn content(&mut self) -> Result<Vec<ContentFile>> {
        let mut found = Vec::new();
        for kind in [
            ContentKind::Mod,
            ContentKind::ResourcePack,
            ContentKind::ShaderPack,
        ] {
            found.extend(scan(
                &self.path,
                kind,
                &Place::Instance,
                &self.config.content,
            )?);
        }
        for world in self.world_folders()? {
            found.extend(scan(
                &self.path,
                ContentKind::DataPack,
                &Place::World(world),
                &self.config.content,
            )?);
        }
        self.config.content = found
            .iter()
            .map(|file| (file.key.clone(), file.record.clone()))
            .collect();
        Ok(found)
    }

    pub fn target(&self, kind: ContentKind, place: &Place, file_name: &str) -> Result<PathBuf> {
        let file_name = checked_name(file_name)?;
        if let Place::World(world) = place {
            checked_name(world)?;
        }
        let folder = content::folder(&self.path, kind, place);
        fs::create_dir_all(&folder).map_err(Error::io(&folder))?;
        Ok(folder.join(file_name))
    }

    #[must_use]
    pub fn key(&self, kind: ContentKind, place: &Place, file_name: &str) -> String {
        content::key(kind, place, file_name)
    }

    pub fn record(&mut self, key: String, record: ContentRecord) {
        self.config.content.insert(key, record);
    }

    pub fn forget(&mut self, key: &str) {
        self.config.content.remove(key);
    }

    pub fn measure(&self, path: &Path) -> Result<ContentRecord> {
        record_for(path, None)
    }

    pub fn import(&mut self, kind: ContentKind, place: &Place, source: &Path) -> Result<String> {
        let name = source
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| Error::FileName(source.display().to_string()))?;
        let target = self.target(kind, place, name)?;
        if Path::new(&format!("{}{DISABLED}", target.display())).exists() {
            return Err(Error::Exists(name.to_owned()));
        }
        let bytes = fs::read(source).map_err(Error::io(source))?;
        let mut staged = rikka_fs::Replace::new(&target)
            .stage()
            .map_err(Error::io(&target))?;
        staged.write_all(&bytes).map_err(Error::io(&target))?;
        match staged.commit_unused([target.clone()]) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(Error::Exists(name.to_owned()));
            }
            Err(source) => {
                return Err(Error::Io {
                    path: target,
                    source,
                });
            }
        }
        let key = self.key(kind, place, name);
        let record = record_for(&target, None)?;
        self.record(key.clone(), record);
        Ok(key)
    }

    pub fn set_enabled(&self, file: &ContentFile, enabled: bool) -> Result<()> {
        if file.enabled == enabled {
            return Ok(());
        }
        let folder = file.path.parent().unwrap_or(&self.path);
        let target = match enabled {
            true => folder.join(&file.file_name),
            false => folder.join(format!("{}{DISABLED}", file.file_name)),
        };
        rikka_fs::rename_new(&file.path, &target).map_err(Error::io(&target))
    }

    pub fn disable(&self, kind: ContentKind, place: &Place, file_name: &str) -> Result<()> {
        let file_name = checked_name(file_name)?;
        let folder = content::folder(&self.path, kind, place);
        let disabled = folder.join(format!("{file_name}{DISABLED}"));
        rikka_fs::rename_new(&folder.join(file_name), &disabled).map_err(Error::io(&disabled))
    }

    pub fn remove_content(&mut self, file: &ContentFile) -> Result<()> {
        fs::remove_file(&file.path).map_err(Error::io(&file.path))?;
        self.forget(&file.key);
        Ok(())
    }

    pub fn worlds(&self) -> Result<Worlds> {
        worlds::worlds(&self.path)
    }

    pub fn remove_world(&self, folder: &str) -> Result<()> {
        let path = self.path.join(SAVES).join(checked_name(folder)?);
        fs::remove_dir_all(&path).map_err(Error::io(&path))
    }

    pub fn servers(&self) -> Result<Vec<Server>> {
        worlds::servers(&self.path)
    }

    pub fn add_server(&self, name: &str, address: &str) -> Result<()> {
        worlds::add_server(&self.path, name, address)
    }

    pub fn remove_server(&self, address: &str) -> Result<()> {
        worlds::remove_server(&self.path, address)
    }

    pub fn screenshots(&self) -> Result<Vec<Screenshot>> {
        files::screenshots(&self.path)
    }

    pub fn logs(&self) -> Result<Vec<LogFile>> {
        files::logs(&self.path)
    }
}
