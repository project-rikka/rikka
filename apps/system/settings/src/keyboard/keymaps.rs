use std::path::Path;

const ROOT: &str = "/run/current-system/sw/share/keymaps";

const DEPTH: usize = 4;

pub(crate) fn installed() -> Vec<String> {
    let mut keymaps = Vec::new();
    walk(Path::new(ROOT), DEPTH, &mut keymaps);
    keymaps.sort_unstable();
    keymaps.dedup();
    keymaps
}

fn walk(directory: &Path, depth: usize, keymaps: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if depth > 1 {
                walk(&path, depth - 1, keymaps);
            }
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if let Some(keymap) = named(&name) {
            keymaps.push(keymap);
        }
    }
}

fn named(file: &str) -> Option<String> {
    let (stem, rest) = file.split_once('.')?;
    if stem.is_empty() || !matches!(rest, "map" | "map.gz" | "kmap" | "kmap.gz") {
        return None;
    }
    Some(stem.to_owned())
}
