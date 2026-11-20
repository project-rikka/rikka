use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

use rikka_app::LaunchRequest;

const EXECUTE: &str = "-e";
const END_OF_OPTIONS: &str = "--";
const DIR: &str = "--dir=";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Opening {
    pub(crate) dir: Option<PathBuf>,
    pub(crate) program: Option<Result<Program, Unreadable>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Program {
    pub(crate) name: String,
    pub(crate) arguments: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Unreadable(OsString);

impl Opening {
    pub(crate) fn from_request(request: &LaunchRequest) -> Opening {
        let mut opening = Opening {
            dir: request.cwd().map(PathBuf::from),
            program: None,
        };
        let mut arguments = request.arguments().iter();
        while let Some(argument) = arguments.next() {
            if argument == EXECUTE || argument == END_OF_OPTIONS {
                let words: Result<Vec<String>, Unreadable> = arguments
                    .map(|word| word.clone().into_string().map_err(Unreadable))
                    .collect();
                opening.program = match words {
                    Ok(words) => words.split_first().map(|(name, arguments)| {
                        Ok(Program {
                            name: name.clone(),
                            arguments: arguments.to_vec(),
                        })
                    }),
                    Err(unreadable) => Some(Err(unreadable)),
                };
                break;
            }
            let dir = argument
                .as_bytes()
                .strip_prefix(DIR.as_bytes())
                .filter(|dir| !dir.is_empty())
                .map(OsStr::from_bytes);
            if let Some(dir) = dir {
                opening.dir = Some(match request.cwd() {
                    Some(cwd) => cwd.join(dir),
                    None => PathBuf::from(dir),
                });
            }
        }
        opening
    }
}

impl std::fmt::Display for Unreadable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cannot run a command that is not UTF-8: {:?}", self.0)
    }
}
