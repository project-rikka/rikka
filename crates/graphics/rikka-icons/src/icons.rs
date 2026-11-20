use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rikka_sync::Mutex;

use crate::dirs;
use crate::theme::{self, Theme};

pub const FALLBACK_THEME: &str = "hicolor";

const MAX_THEMES: usize = 16;

type Cache = HashMap<String, HashMap<(u32, u32), Option<Arc<Path>>>>;

const UNREAD_EXTENSION: &str = "xpm";

pub struct Icons {
    theme: String,
    theme_roots: Vec<PathBuf>,
    pixmap_roots: Vec<PathBuf>,
    chain: Vec<Theme>,
    cache: Mutex<Cache>,
}

impl Icons {
    #[must_use]
    pub fn new(theme: &str) -> Icons {
        let theme_roots = dirs::theme_roots();
        let chain = chain(theme, &theme_roots);
        Icons {
            theme: theme.to_owned(),
            theme_roots,
            pixmap_roots: dirs::pixmap_roots(),
            chain,
            cache: Mutex::default(),
        }
    }

    #[must_use]
    pub fn with_root(root: &Path) -> Icons {
        let theme_roots = vec![root.to_path_buf()];
        Icons {
            theme: FALLBACK_THEME.to_owned(),
            chain: chain(FALLBACK_THEME, &theme_roots),
            pixmap_roots: theme_roots.clone(),
            theme_roots,
            cache: Mutex::default(),
        }
    }

    #[must_use]
    pub fn theme(&self) -> &str {
        &self.theme
    }

    pub fn set_theme(&mut self, theme: &str) {
        self.theme = theme.to_owned();
        self.theme_roots = dirs::theme_roots();
        self.pixmap_roots = dirs::pixmap_roots();
        self.chain = chain(theme, &self.theme_roots);
        self.clear_cache();
    }

    pub fn clear_cache(&mut self) {
        self.cache.get_mut().clear();
        for theme in &mut self.chain {
            theme.clear_cache();
        }
    }

    #[must_use]
    pub fn lookup(&self, name: &str, size: u32, scale: u32) -> Option<Arc<Path>> {
        let scale = scale.max(1);
        if let Some(icon) = self
            .cache
            .lock()
            .get(name)
            .and_then(|sizes| sizes.get(&(size, scale)))
        {
            return icon.clone();
        }
        let icon = self.find(name, size, scale);
        self.cache
            .lock()
            .entry(name.to_owned())
            .or_default()
            .insert((size, scale), icon.clone());
        icon
    }

    fn find(&self, name: &str, size: u32, scale: u32) -> Option<Arc<Path>> {
        let named = Path::new(name);
        if named.is_absolute() {
            return named.is_file().then(|| Arc::from(named));
        }
        if name.is_empty() || name.contains(['/', '\\']) {
            return None;
        }

        let name = strip_extension(name);
        for theme in &self.chain {
            if let Some(icon) = theme.lookup(name, size, scale) {
                return Some(icon);
            }
        }
        theme::lookup_pixmap(&self.pixmap_roots, name)
    }
}

impl std::fmt::Debug for Icons {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Icons")
            .field("theme", &self.theme)
            .field("themes", &self.chain.len())
            .finish_non_exhaustive()
    }
}

fn chain(name: &str, bases: &[PathBuf]) -> Vec<Theme> {
    let mut chain: Vec<Theme> = Vec::new();
    let mut visited: Vec<String> = Vec::new();
    let mut queue: VecDeque<String> = VecDeque::from([name.to_owned(), FALLBACK_THEME.to_owned()]);

    while let Some(name) = queue.pop_front() {
        if visited.contains(&name) || visited.len() >= MAX_THEMES {
            continue;
        }
        visited.push(name.clone());
        let Some(theme) = Theme::read(&name, bases) else {
            continue;
        };
        for (offset, parent) in theme.inherits().iter().enumerate() {
            queue.insert(offset, parent.clone());
        }
        chain.push(theme);
    }

    if chain.is_empty() {
        tracing::debug!("neither icon theme {name} nor {FALLBACK_THEME} is installed");
    }
    chain
}

fn strip_extension(name: &str) -> &str {
    let Some((stem, extension)) = name.rsplit_once('.') else {
        return name;
    };
    match theme::EXTENSIONS
        .iter()
        .chain([&UNREAD_EXTENSION])
        .any(|extension_name| extension.eq_ignore_ascii_case(extension_name))
    {
        true => stem,
        false => name,
    }
}
