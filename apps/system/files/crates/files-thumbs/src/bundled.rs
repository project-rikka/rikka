use std::path::{Path, PathBuf};
use std::process::Command;

use files_core::Family;
use rikka_mime::MimeType;
use rikka_open::{find, is_executable};

const PROGRAM: &str = "rikka-video-thumbnailer";

#[derive(Clone, Debug)]
pub(crate) struct Bundled {
    program: PathBuf,
}

impl Bundled {
    pub(crate) fn locate() -> Option<Bundled> {
        std::env::current_exe()
            .ok()
            .and_then(|running| Some(running.parent()?.join(PROGRAM)))
            .filter(|beside| is_executable(beside))
            .or_else(|| find(PROGRAM))
            .map(|program| Bundled { program })
    }

    pub(crate) fn handles(mime: &MimeType) -> bool {
        Family::from_mime(mime) == Family::Video
    }

    pub(crate) fn command(&self, input: &Path, output: &Path, size: u32) -> Command {
        let mut command = Command::new(&self.program);
        command
            .arg("-s")
            .arg(size.to_string())
            .arg("--")
            .arg(input)
            .arg(output);
        command
    }
}
