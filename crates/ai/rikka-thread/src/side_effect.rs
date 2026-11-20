use std::io::{Read, Seek};

use rikka_file_edit::Change;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SideEffect {
    FileChange(Change),
    Output(OutputLog),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputLog {
    pub at: String,
}

impl SideEffect {
    #[must_use]
    pub fn file_change(&self) -> Option<&Change> {
        match self {
            SideEffect::FileChange(change) => Some(change),
            SideEffect::Output(_) => None,
        }
    }

    #[must_use]
    pub fn output_log(&self) -> Option<&OutputLog> {
        match self {
            SideEffect::Output(printout) => Some(printout),
            SideEffect::FileChange(_) => None,
        }
    }
}

impl OutputLog {
    pub fn read(&self, most: usize) -> std::io::Result<String> {
        let mut file = std::fs::File::open(&self.at)?;
        let length = file.metadata()?.len();
        let from = length.saturating_sub(u64::try_from(most).unwrap_or(u64::MAX));
        file.seek(std::io::SeekFrom::Start(from))?;

        let mut tail = Vec::new();
        file.read_to_end(&mut tail)?;

        let past = match from {
            0 => 0,
            _ => tail
                .iter()
                .position(|byte| *byte == b'\n')
                .map_or(0, |line| line + 1),
        };
        let text = String::from_utf8_lossy(&tail[past..]);
        let text = text.trim_end();
        Ok(match from {
            0 => text.to_owned(),
            from => {
                let earlier = from + u64::try_from(past).unwrap_or_default();
                format!("… [{earlier} bytes earlier in {}] …\n{text}", self.at)
            }
        })
    }
}
