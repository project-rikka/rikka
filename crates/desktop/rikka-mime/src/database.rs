use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use crate::builtin;
use crate::comment;
use crate::glob::Globs;
use crate::graph::Graph;
use crate::magic::{Magic, SNIFF_BYTES};
use crate::mime_type::MimeType;

#[derive(Debug)]
pub struct Database {
    globs: Globs,
    magic: Magic,
    graph: Graph,
    dirs: Vec<PathBuf>,
    reading: RwLock<Reading>,
    descriptions: RwLock<HashMap<MimeType, Option<String>>>,
}

impl Database {
    #[must_use]
    pub fn load() -> Self {
        Self::assemble(rikka_dirs::data_dirs(), cfg!(windows))
    }

    #[must_use]
    pub fn from_dirs(dirs: Vec<PathBuf>) -> Self {
        Self::assemble(dirs, false)
    }

    #[must_use]
    pub fn builtin() -> Self {
        Self::assemble(Vec::new(), true)
    }

    fn assemble(dirs: Vec<PathBuf>, builtin: bool) -> Self {
        let mut globs = Globs::default();
        let mut magic = Magic::default();
        let mut graph = Graph::default();

        for dir in &dirs {
            let mime = dir.join("mime");
            if let Ok(text) = std::fs::read_to_string(mime.join("globs2")) {
                globs.extend_from(&text);
            }
            if let Ok(bytes) = std::fs::read(mime.join("magic")) {
                magic.extend_from(&bytes);
            }
            if let Ok(text) = std::fs::read_to_string(mime.join("aliases")) {
                graph.extend_aliases(&text);
            }
            if let Ok(text) = std::fs::read_to_string(mime.join("subclasses")) {
                graph.extend_subclasses(&text);
            }
        }
        if builtin {
            globs.extend_from(builtin::GLOBS);
            magic.extend_from(&builtin::magic());
            graph.extend_aliases(builtin::ALIASES);
            graph.extend_subclasses(builtin::SUBCLASSES);
        }

        Self {
            globs,
            magic,
            graph,
            dirs,
            reading: RwLock::new(Reading::new(String::new())),
            descriptions: RwLock::new(HashMap::new()),
        }
    }

    #[must_use]
    pub fn globs_for(&self, mime: &str) -> Vec<String> {
        let kind = mime.trim().strip_suffix("/*");
        let exact = MimeType::parse(mime).map(|parsed| self.graph.resolve(&parsed));
        let mut globs: Vec<String> = self
            .globs
            .patterns()
            .filter(|(_, candidate)| match (kind, &exact) {
                (Some(kind), _) => candidate.kind().eq_ignore_ascii_case(kind),
                (None, Some(exact)) => self.graph.ancestry(candidate).contains(exact),
                (None, None) => false,
            })
            .map(|(glob, _)| glob.to_owned())
            .collect();
        globs.sort();
        globs.dedup();
        globs
    }

    #[must_use]
    pub fn for_file(&self, path: &Path) -> MimeType {
        let named = self.for_name(path);
        if named.len() == 1 {
            return named.into_iter().next().unwrap_or_else(Self::unknown);
        }
        match head(path) {
            Some(head) => self.for_content(&head, &named),
            None => named.into_iter().next().unwrap_or_else(Self::unknown),
        }
    }

    #[must_use]
    pub fn for_name(&self, path: &Path) -> Vec<MimeType> {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            return Vec::new();
        };
        self.globs
            .lookup(name)
            .iter()
            .map(|mime| self.graph.resolve(mime))
            .collect()
    }

    #[must_use]
    pub fn for_content(&self, head: &[u8], named: &[MimeType]) -> MimeType {
        if head.is_empty() {
            return MimeType::well_known(MimeType::EMPTY);
        }
        if let Some(detected) = self
            .magic
            .lookup(head, named, |mime| self.graph.resolve(mime))
        {
            return detected;
        }
        if let Some(named) = named.first() {
            return named.clone();
        }
        if is_text(head) {
            MimeType::well_known(MimeType::TEXT)
        } else {
            Self::unknown()
        }
    }

    #[must_use]
    pub fn ancestry(&self, mime: &MimeType) -> Vec<MimeType> {
        self.graph.ancestry(mime)
    }

    #[must_use]
    pub fn resolve(&self, mime: &MimeType) -> MimeType {
        self.graph.resolve(mime)
    }

    pub fn set_language(&self, tag: &str) {
        if self.reading.read().is_ok_and(|reading| reading.tag == tag) {
            return;
        }
        let Ok(mut reading) = self.reading.write() else {
            return;
        };
        if reading.tag == tag {
            return;
        }
        *reading = Reading::new(tag.to_owned());
        if let Ok(mut cache) = self.descriptions.write() {
            cache.clear();
        }
    }

    #[must_use]
    pub fn describe(&self, mime: &MimeType) -> Option<String> {
        if let Some(known) = self
            .descriptions
            .read()
            .ok()
            .and_then(|cache| cache.get(mime).cloned())
        {
            return known;
        }
        let locales = self.reading.read().ok()?.locales.clone();
        let described = comment::read(&self.dirs, mime, &locales);
        if let Ok(mut cache) = self.descriptions.write() {
            cache.insert(mime.clone(), described.clone());
        }
        described
    }

    fn unknown() -> MimeType {
        MimeType::well_known(MimeType::UNKNOWN)
    }
}

#[derive(Debug)]
struct Reading {
    tag: String,
    locales: Vec<String>,
}

impl Reading {
    fn new(tag: String) -> Self {
        let locales = comment::locales_of(&tag);
        Self { tag, locales }
    }
}

fn head(path: &Path) -> Option<Vec<u8>> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut buffer = vec![0; SNIFF_BYTES];
    let mut filled = 0;
    loop {
        match file.read(&mut buffer[filled..]) {
            Ok(0) => break,
            Ok(read) => {
                filled += read;
                if filled == buffer.len() {
                    break;
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return None,
        }
    }
    buffer.truncate(filled);
    Some(buffer)
}

fn is_text(head: &[u8]) -> bool {
    !head
        .iter()
        .take(SNIFF_BYTES.min(4096))
        .any(|&byte| byte == 0)
}
