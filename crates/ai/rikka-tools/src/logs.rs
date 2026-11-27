use rikka_sync::Mutex;
use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

const FOLDER: [&str; 2] = [".rikka", "tmp"];

const SUFFIX: &str = ".log";

const UNSEEN: &str = "*\n";

const LONGEST: usize = 64 * 1024 * 1024;

const ALTOGETHER: u64 = 512 * 1024 * 1024;

const TRIES: usize = 64;

#[derive(Clone, Debug)]
pub(crate) struct Logs {
    folder: PathBuf,
    root: PathBuf,
    retain: usize,
    live: Arc<Mutex<HashSet<PathBuf>>>,
}

impl Logs {
    pub(crate) fn new(root: &Path, retain: usize) -> Logs {
        Logs {
            folder: FOLDER
                .iter()
                .fold(root.to_owned(), |at, part| at.join(part)),
            root: root.to_owned(),
            retain: retain.max(1),
            live: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub(crate) fn folder(&self) -> &Path {
        &self.folder
    }

    pub(crate) fn open(&self, named: &str) -> Option<Log> {
        self.made().ok()?;

        let stamp = jiff::Zoned::now().strftime("%Y%m%d-%H%M%S").to_string();

        for nth in 0..TRIES {
            let name = match nth {
                0 => format!("{stamp}-{named}{SUFFIX}"),
                nth => format!("{stamp}-{named}-{nth}{SUFFIX}"),
            };
            let at = self.folder.join(name);

            match OpenOptions::new().write(true).create_new(true).open(&at) {
                Ok(file) => {
                    self.live.lock().insert(at.clone());
                    self.prune();
                    let display_path = at.strip_prefix(&self.root).unwrap_or(&at);
                    return Some(Log::new(
                        file,
                        at.clone(),
                        display_path.display().to_string(),
                        Arc::clone(&self.live),
                    ));
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(_) => return None,
            }
        }

        None
    }

    fn made(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.folder)?;

        let unseen = self.folder.join(".gitignore");
        match unseen.exists() {
            true => Ok(()),
            false => std::fs::write(unseen, UNSEEN),
        }
    }

    fn prune(&self) {
        let Ok(reading) = std::fs::read_dir(&self.folder) else {
            return;
        };

        let live = self.live.lock().clone();
        let mut logs: Vec<(SystemTime, u64, PathBuf)> = reading
            .flatten()
            .filter(|entry| entry.path().extension().is_some_and(|end| end == "log"))
            .filter_map(|entry| {
                let about = entry.metadata().ok()?;
                Some((about.modified().ok()?, about.len(), entry.path()))
            })
            .collect();

        logs.sort_unstable_by_key(|(logs, _, _)| std::cmp::Reverse(*logs));

        let mut total = 0u64;
        for (nth, (_, bytes, at)) in logs.into_iter().enumerate() {
            total += bytes;
            if (nth >= self.retain || total > ALTOGETHER) && !live.contains(&at) {
                let _ = std::fs::remove_file(at);
            }
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Log {
    at: PathBuf,
    display_path: String,
    writing: Arc<Mutex<Writing>>,
}

#[derive(Debug)]
struct Writing {
    file: Option<File>,
    bytes_written: usize,
    at: PathBuf,
    live: Arc<Mutex<HashSet<PathBuf>>>,
}

impl Log {
    fn new(
        file: File,
        at: PathBuf,
        display_path: String,
        live: Arc<Mutex<HashSet<PathBuf>>>,
    ) -> Log {
        Log {
            display_path,
            writing: Arc::new(Mutex::new(Writing {
                file: Some(file),
                bytes_written: 0,
                at: at.clone(),
                live,
            })),
            at,
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.at
    }

    pub(crate) fn display_path(&self) -> &str {
        &self.display_path
    }

    pub(crate) fn bytes(&self) -> usize {
        self.writing.lock().bytes_written
    }

    pub(crate) fn wrote(&self, bytes: &[u8]) {
        self.writing.lock().wrote(bytes);
    }

    pub(crate) fn forget(&self) {
        self.writing.lock().file = None;
        let _ = std::fs::remove_file(&self.at);
    }
}

impl Writing {
    fn wrote(&mut self, bytes: &[u8]) {
        let Some(file) = &mut self.file else {
            return;
        };

        let room = LONGEST.saturating_sub(self.bytes_written);
        let length = room.min(bytes.len());
        if file.write_all(&bytes[..length]).is_err() {
            self.file = None;
            return;
        }
        self.bytes_written += length;

        if length < bytes.len() {
            let cut = cut();
            if file.write_all(cut.as_bytes()).is_ok() {
                self.bytes_written += cut.len();
            }
            self.file = None;
        }
    }
}

impl Drop for Writing {
    fn drop(&mut self) {
        self.live.lock().remove(&self.at);
    }
}

fn cut() -> String {
    format!("\n… [nothing past {LONGEST} bytes was kept] …\n")
}
