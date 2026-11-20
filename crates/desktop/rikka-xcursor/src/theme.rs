use std::path::{Path, PathBuf};

const FALLBACK: &str = "default";

const SEARCH_ENV: &str = "XCURSOR_PATH";

const INDEX: &str = "index.theme";

const CURSORS: &str = "cursors";

const INHERITS: &str = "Inherits";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Theme {
    folders: Vec<PathBuf>,
}

impl Theme {
    #[must_use]
    pub fn load(name: &str) -> Theme {
        Theme::load_from(name, &search_path())
    }

    #[must_use]
    pub fn load_from(name: &str, search: &[PathBuf]) -> Theme {
        let mut theme = Theme::default();
        let mut seen = Vec::new();
        theme.gather(name, search, &mut seen);
        theme.gather(FALLBACK, search, &mut seen);
        theme
    }

    #[must_use]
    pub fn find(&self, cursor: &str) -> Option<PathBuf> {
        self.folders
            .iter()
            .map(|folder| folder.join(cursor))
            .find(|path| path.is_file())
    }

    fn gather(&mut self, name: &str, search: &[PathBuf], seen: &mut Vec<String>) {
        if name.is_empty() || name.contains('/') || seen.iter().any(|known| known == name) {
            return;
        }
        seen.push(name.to_owned());
        let mut inherited = None;
        for root in search {
            let theme = root.join(name);
            let cursors = theme.join(CURSORS);
            if cursors.is_dir() {
                self.folders.push(cursors);
            }
            if inherited.is_none() {
                inherited = parents(&theme.join(INDEX));
            }
        }
        for parent in inherited.unwrap_or_default() {
            self.gather(&parent, search, seen);
        }
    }
}

fn parents(index: &Path) -> Option<Vec<String>> {
    let index = std::fs::read_to_string(index).ok()?;
    let listed = index.lines().find_map(|line| {
        let (key, value) = line.split_once('=')?;
        (key.trim() == INHERITS).then_some(value)
    })?;
    Some(
        listed
            .split([',', ';', ':'])
            .map(str::trim)
            .filter(|parent| !parent.is_empty())
            .map(str::to_owned)
            .collect(),
    )
}

fn search_path() -> Vec<PathBuf> {
    if let Some(listed) = std::env::var_os(SEARCH_ENV).filter(|listed| !listed.is_empty()) {
        return std::env::split_paths(&listed).map(expanded).collect();
    }
    let home = rikka_dirs::home();
    rikka_dirs::data_home()
        .map(|data| data.join("icons"))
        .into_iter()
        .chain(home.map(|home| home.join(".icons")))
        .chain(
            rikka_dirs::data_dirs()
                .into_iter()
                .map(|data| data.join("icons")),
        )
        .chain([PathBuf::from("/usr/share/pixmaps")])
        .fold(Vec::new(), |mut search, folder| {
            if !search.contains(&folder) {
                search.push(folder);
            }
            search
        })
}

fn expanded(listed: PathBuf) -> PathBuf {
    match (listed.strip_prefix("~"), rikka_dirs::home()) {
        (Ok(rest), Some(home)) => home.join(rest),
        _ => listed,
    }
}
