use std::fs;
use std::io::{self, Read as _, Seek as _, SeekFrom};
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use rikka_nix_protocol::{Operation, PageRecord, Run, RunId, RunRecord, RunState, Status};

use crate::error::{Error, Result};
use crate::transcript::Transcript;

pub const KEPT_RUNS: usize = 10;

pub const PAGE_BYTES: u64 = 1024 * 1024;

const FOLDER_MODE: u32 = 0o700;

const RECORD: &str = "json";

const LOG: &str = "log";

#[derive(Debug)]
pub struct Journal {
    folder: PathBuf,
    runs: Vec<Run>,
}

#[derive(Debug)]
pub struct Opened {
    pub journal: Journal,
    pub rejected: Vec<Error>,
}

impl Journal {
    pub fn open(folder: impl Into<PathBuf>) -> Result<Opened> {
        let folder = folder.into();
        let write = |source| Error::Write {
            path: folder.clone(),
            source,
        };
        fs::create_dir_all(&folder).map_err(write)?;
        fs::set_permissions(&folder, fs::Permissions::from_mode(FOLDER_MODE)).map_err(write)?;

        let (mut runs, rejected) = read_runs(&folder)?;
        runs.sort_by_key(|run| run.id);
        let mut journal = Self { folder, runs };
        journal.remove_orphan_logs()?;
        for at in 0..journal.runs.len() {
            if !journal.runs[at].is_running() {
                continue;
            }
            let stopped = fs::metadata(journal.path(journal.runs[at].id, LOG))
                .and_then(|metadata| metadata.modified())
                .ok();
            let run = &mut journal.runs[at];
            run.interrupt(stopped.unwrap_or(run.started));
            journal.write(&journal.runs[at])?;
        }
        Ok(Opened { journal, rejected })
    }

    pub fn runs(&self) -> impl DoubleEndedIterator<Item = &Run> {
        self.runs.iter().rev()
    }

    #[must_use]
    pub fn run(&self, id: RunId) -> Option<&Run> {
        self.runs.iter().find(|run| run.id == id)
    }

    pub fn begin(&mut self, operation: Operation) -> Result<(Run, Transcript)> {
        let started = SystemTime::now();
        let id = self
            .runs
            .last()
            .map_or(RunId::new(0), |run| RunId::new(run.id.get() + 1))
            .max(RunId::from(started));
        let transcript = Transcript::create(self.path(id, LOG))?;
        let run = Run {
            id,
            operation,
            started,
            finished: None,
            state: RunState::Running,
            status: Status::default(),
        };
        if let Err(error) = self.write(&run) {
            remove_if_present(transcript.path())?;
            return Err(error);
        }
        self.runs.push(run.clone());
        self.prune()?;
        Ok((run, transcript))
    }

    pub fn update(&mut self, id: RunId, status: Status) -> Result<()> {
        self.run_mut(id)?.status = status;
        Ok(())
    }

    pub fn finish(&mut self, id: RunId, state: RunState) -> Result<Run> {
        let run = self.run_mut(id)?;
        run.state = state;
        run.finished = Some(SystemTime::now());
        let finished = run.clone();
        self.write(&finished)?;
        Ok(finished)
    }

    pub fn page(&self, id: RunId, from: u64) -> Result<PageRecord> {
        if !self.runs.iter().any(|run| run.id == id) {
            return Err(Error::UnknownRun(id));
        }
        let path = self.path(id, LOG);
        let read = |source| Error::Read {
            path: path.clone(),
            source,
        };
        let mut file = fs::File::open(&path).map_err(read)?;
        let length = file.metadata().map_err(read)?.len();
        let misplaced = Error::Offset {
            run: id,
            offset: from,
        };
        if from > length {
            return Err(misplaced);
        }
        if from > 0 {
            let mut before = [0u8];
            file.seek(SeekFrom::Start(from - 1)).map_err(read)?;
            file.read_exact(&mut before).map_err(read)?;
            if before[0] != b'\n' {
                return Err(misplaced);
            }
        }
        file.seek(SeekFrom::Start(from)).map_err(read)?;
        let mut bytes = Vec::new();
        file.take(PAGE_BYTES)
            .read_to_end(&mut bytes)
            .map_err(read)?;
        let end = (bytes.len() as u64) < PAGE_BYTES;
        let whole = bytes
            .iter()
            .rposition(|byte| *byte == b'\n')
            .map_or(0, |at| at + 1);
        let lines = bytes[..whole]
            .split_inclusive(|byte| *byte == b'\n')
            .map(|line| String::from_utf8_lossy(&line[..line.len() - 1]).into_owned())
            .collect();
        Ok(PageRecord {
            lines,
            next: from + whole as u64,
            end,
        })
    }

    fn prune(&mut self) -> Result<()> {
        while self.runs.len() > KEPT_RUNS {
            let Some(oldest) = self.runs.iter().position(|run| !run.is_running()) else {
                return Ok(());
            };
            let id = self.runs.remove(oldest).id;
            for extension in [RECORD, LOG] {
                remove_if_present(&self.path(id, extension))?;
            }
        }
        Ok(())
    }

    fn write(&self, run: &Run) -> Result<()> {
        let path = self.path(run.id, RECORD);
        let json =
            serde_json::to_vec_pretty(&RunRecord::from(run)).map_err(|error| Error::Record {
                path: path.clone(),
                reason: error.to_string(),
            })?;
        rikka_fs::replace(&path, json).map_err(|source| Error::Write { path, source })
    }

    fn remove_orphan_logs(&self) -> Result<()> {
        let read = |source| Error::Read {
            path: self.folder.clone(),
            source,
        };
        for entry in fs::read_dir(&self.folder).map_err(read)? {
            let path = entry.map_err(read)?.path();
            let is_log = path.extension().and_then(|extension| extension.to_str()) == Some(LOG);
            if is_log && !path.with_extension(RECORD).exists() {
                remove_if_present(&path)?;
            }
        }
        Ok(())
    }

    fn run_mut(&mut self, id: RunId) -> Result<&mut Run> {
        self.runs
            .iter_mut()
            .find(|run| run.id == id)
            .ok_or(Error::UnknownRun(id))
    }

    fn path(&self, id: RunId, extension: &str) -> PathBuf {
        self.folder.join(format!("{id}.{extension}"))
    }
}

fn read_runs(folder: &Path) -> Result<(Vec<Run>, Vec<Error>)> {
    let read = |source| Error::Read {
        path: folder.to_path_buf(),
        source,
    };
    let mut runs = Vec::new();
    let mut rejected = Vec::new();
    for entry in fs::read_dir(folder).map_err(read)? {
        let path = entry.map_err(read)?.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some(RECORD) {
            continue;
        }
        match read_run(&path) {
            Ok(run) => runs.push(run),
            Err(error) => rejected.push(error),
        }
    }
    Ok((runs, rejected))
}

fn read_run(path: &Path) -> Result<Run> {
    let bytes = fs::read(path).map_err(|source| Error::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let malformed = |reason: String| Error::Record {
        path: path.to_path_buf(),
        reason,
    };
    let record: RunRecord =
        serde_json::from_slice(&bytes).map_err(|error| malformed(error.to_string()))?;
    Run::try_from(record).map_err(|error| malformed(error.to_string()))
}

fn remove_if_present(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Err(source) if source.kind() != io::ErrorKind::NotFound => Err(Error::Write {
            path: path.to_path_buf(),
            source,
        }),
        _ => Ok(()),
    }
}
