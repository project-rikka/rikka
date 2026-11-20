use crate::error::Error;
use crate::git::error_message;
use crate::progress::is_phase;

const REJECTIONS: [&str; 2] = ["(fetch first)", "(non-fast-forward)"];

const SKIPPED: [&str; 3] = ["hint:", "From ", "To "];

const CHATTER: [&str; 2] = ["Total ", "Delta compression"];

#[derive(Debug)]
pub(crate) enum Failure {
    Unavailable(Error),
    Refused {
        said: Vec<String>,
        printed: String,
        code: Option<i32>,
    },
}

impl Failure {
    pub(crate) fn is_rejected(&self) -> bool {
        let Failure::Refused { said, .. } = self else {
            return false;
        };
        said.iter().any(|line| {
            line.trim_start().starts_with("! [rejected]")
                && REJECTIONS.iter().any(|reason| line.contains(reason))
        })
    }

    pub(crate) fn into_error(self) -> Error {
        match self {
            Failure::Unavailable(error) => error,
            Failure::Refused {
                said,
                printed,
                code,
            } => {
                let telling: Vec<&str> = said
                    .iter()
                    .map(String::as_str)
                    .filter(|line| is_telling(line))
                    .collect();
                Error::Refused(error_message(
                    telling.join("\n").as_bytes(),
                    printed.as_bytes(),
                    code,
                ))
            }
        }
    }
}

fn is_telling(line: &str) -> bool {
    if SKIPPED.iter().any(|skipped| line.starts_with(skipped)) {
        return false;
    }
    let spoken = line.strip_prefix("remote:").map_or(line, str::trim);
    if spoken.is_empty()
        || is_phase(spoken)
        || CHATTER.iter().any(|chatter| spoken.starts_with(chatter))
    {
        return false;
    }
    !line.starts_with(' ') || line.trim_start().starts_with('!')
}
