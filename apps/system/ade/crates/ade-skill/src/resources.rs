use std::path::Path;

const MOST: usize = 100;

const DEEP: usize = 3;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Resources {
    files: Vec<String>,
    more: bool,
}

impl Resources {
    pub(crate) fn beside(folder: &Path) -> Resources {
        let mut files = Vec::new();
        walk(folder, folder, DEEP, &mut files);
        files.sort();

        let more = files.len() > MOST;
        files.truncate(MOST);
        Resources { files, more }
    }

    #[must_use]
    pub fn files(&self) -> &[String] {
        &self.files
    }

    #[must_use]
    pub fn is_partial(&self) -> bool {
        self.more
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

fn walk(root: &Path, folder: &Path, deep: usize, files: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };

    for entry in entries.flatten() {
        if files.len() > MOST {
            return;
        }

        let name = entry.file_name();
        if name.as_encoded_bytes().starts_with(b".") {
            continue;
        }

        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };

        if kind.is_dir() {
            if deep > 0 {
                walk(root, &path, deep - 1, files);
            }
            continue;
        }

        if path.parent() == Some(root) && name == "SKILL.md" {
            continue;
        }
        if let Ok(under) = path.strip_prefix(root) {
            files.push(under.display().to_string());
        }
    }
}
