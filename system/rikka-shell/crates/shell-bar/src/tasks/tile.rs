use std::path::PathBuf;

use rikka_session::toplevels::Toplevel;
use shell_apps::{AppIcons, DesktopEntry, owner};
use shell_kit::launch::Spawn;
use task_board::Group;

use crate::tasks::Art;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Tile {
    pub(crate) name: String,
    pub(crate) icon: Art,
    command: Option<Vec<String>>,
    directory: Option<PathBuf>,
}

impl Tile {
    #[must_use]
    pub(crate) fn resolve(group: &Group, entries: &[DesktopEntry], icons: &AppIcons) -> Tile {
        let entry = entries.iter().find(|entry| entry.id == group.key);
        let name = match entry {
            Some(entry) => entry.name.clone(),
            None => group
                .windows
                .first()
                .filter(|_| group.app_id.is_empty())
                .map_or_else(|| group.app_id.clone(), |window| window.title.clone()),
        };
        let icon = icons.for_app(entry, &group.app_id);
        Tile {
            name,
            icon: Art::resolve(icon, icons),
            command: entry.and_then(DesktopEntry::command),
            directory: entry.and_then(|entry| entry.working_dir.clone()),
        }
    }

    #[must_use]
    pub(crate) fn starts(&self) -> Option<Spawn> {
        let mut spawn = Spawn::new(self.command.clone()?);
        spawn.cwd = self.directory.clone();
        Some(spawn)
    }
}

#[must_use]
pub(crate) fn owning_entry(entries: &[DesktopEntry], toplevel: &Toplevel) -> Option<String> {
    owner(entries, &toplevel.app_id)
        .map(|entry| entry.id.clone())
        .or_else(|| (!toplevel.app_id.is_empty()).then(|| toplevel.app_id.clone()))
}
