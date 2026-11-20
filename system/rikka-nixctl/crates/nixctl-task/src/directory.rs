use std::collections::BTreeMap;
use std::io;
use std::os::unix::fs::PermissionsExt as _;
use std::path::PathBuf;

use rikka_nix::Module;
use rikka_nix_protocol::ModuleFile;

use crate::files::{read_if_exists, remove_if_exists};

const DIRECTORY_MODE: u32 = 0o755;

#[derive(Debug)]
pub struct Directory {
    root: PathBuf,
    original: BTreeMap<Module, Option<String>>,
}

impl Directory {
    pub fn open(root: impl Into<PathBuf>) -> io::Result<Self> {
        let root = root.into();
        let original = Module::EVERY
            .into_iter()
            .map(|module| Ok((module, read_if_exists(&root.join(module.file_name()))?)))
            .collect::<io::Result<_>>()?;
        Ok(Self { root, original })
    }

    #[must_use]
    pub fn stale(&self, files: &[ModuleFile]) -> Option<Module> {
        files
            .iter()
            .find(|file| match self.original(file.module) {
                Some(current) => current != file.base,
                None => file.module.blank().source() != file.base,
            })
            .map(|file| file.module)
    }

    pub fn write(&self, files: &[ModuleFile]) -> io::Result<()> {
        std::fs::create_dir_all(&self.root)?;
        std::fs::set_permissions(&self.root, std::fs::Permissions::from_mode(DIRECTORY_MODE))?;

        rikka_fs::replace(self.path(Module::Root), Module::Root.blank().source())?;
        for module in Module::ALL {
            match files.iter().find(|written| written.module == module) {
                Some(written) => rikka_fs::replace(self.path(module), &written.contents)?,
                None if self.original(module).is_none() => {
                    rikka_fs::replace(self.path(module), module.blank().source())?;
                }
                None => {}
            }
        }
        Ok(())
    }

    pub fn restore(&self) -> io::Result<()> {
        for (module, original) in &self.original {
            let path = self.path(*module);
            match original {
                Some(contents) => rikka_fs::replace(&path, contents)?,
                None => remove_if_exists(&path)?,
            }
        }
        Ok(())
    }

    fn original(&self, module: Module) -> Option<&str> {
        self.original.get(&module)?.as_deref()
    }

    fn path(&self, module: Module) -> PathBuf {
        self.root.join(module.file_name())
    }
}
