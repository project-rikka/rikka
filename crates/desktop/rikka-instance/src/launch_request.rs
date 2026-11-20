use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const END_OF_OPTIONS: &str = "--";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LaunchRequest {
    arguments: Vec<OsString>,
    uris: Vec<String>,
    action: Option<String>,
    #[cfg(unix)]
    token: Option<String>,
    cwd: Option<PathBuf>,
}

impl LaunchRequest {
    #[must_use]
    pub fn from_arguments(arguments: impl IntoIterator<Item = OsString>) -> LaunchRequest {
        LaunchRequest::from_command_line(
            arguments.into_iter().skip(1).collect(),
            std::env::current_dir().ok(),
        )
    }

    pub(crate) fn from_command_line(
        arguments: Vec<OsString>,
        cwd: Option<PathBuf>,
    ) -> LaunchRequest {
        let uris = operands(&arguments)
            .map(|operand| uri(operand, cwd.as_deref()))
            .collect();
        LaunchRequest {
            arguments,
            uris,
            action: None,
            #[cfg(unix)]
            token: None,
            cwd,
        }
    }

    #[must_use]
    pub fn from_uris(uris: Vec<String>) -> LaunchRequest {
        LaunchRequest {
            uris,
            ..LaunchRequest::default()
        }
    }

    #[must_use]
    pub fn from_action(name: impl Into<String>) -> LaunchRequest {
        LaunchRequest {
            action: Some(name.into()),
            ..LaunchRequest::default()
        }
    }

    #[cfg(unix)]
    #[must_use]
    pub fn with_token(mut self, token: impl Into<String>) -> LaunchRequest {
        self.token = Some(token.into());
        self
    }

    #[must_use]
    pub fn with_cwd(mut self, cwd: impl Into<PathBuf>) -> LaunchRequest {
        self.cwd = Some(cwd.into());
        self
    }

    #[must_use]
    pub fn arguments(&self) -> &[OsString] {
        &self.arguments
    }

    #[must_use]
    pub fn has_flag(&self, flag: &str) -> bool {
        self.arguments
            .iter()
            .take_while(|argument| *argument != END_OF_OPTIONS)
            .any(|argument| argument == flag)
    }

    #[must_use]
    pub fn uris(&self) -> &[String] {
        &self.uris
    }

    #[must_use]
    pub fn paths(&self) -> Vec<PathBuf> {
        self.uris
            .iter()
            .filter_map(|uri| rikka_open::uri_to_path(uri))
            .collect()
    }

    #[must_use]
    pub fn action(&self) -> Option<&str> {
        self.action.as_deref()
    }

    #[cfg(unix)]
    #[must_use]
    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    #[must_use]
    pub fn cwd(&self) -> Option<&Path> {
        self.cwd.as_deref()
    }

    #[must_use]
    pub fn is_bare(&self) -> bool {
        self.arguments.is_empty() && self.uris.is_empty() && self.action.is_none()
    }
}

fn operands(arguments: &[OsString]) -> impl Iterator<Item = &OsStr> {
    let mut options = true;
    arguments.iter().filter_map(move |argument| {
        if options && argument == END_OF_OPTIONS {
            options = false;
            return None;
        }
        let bytes = argument.as_encoded_bytes();
        let option = options && bytes.len() > 1 && bytes[0] == b'-';
        (!option).then_some(argument.as_os_str())
    })
}

pub type Sink = Arc<dyn Fn(LaunchRequest) + Send + Sync>;

fn uri(argument: &OsStr, cwd: Option<&Path>) -> String {
    if let Some(text) = argument.to_str()
        && has_scheme(text)
    {
        return text.to_owned();
    }
    let path = Path::new(argument);
    let absolute = match (path.is_absolute(), cwd) {
        (true, _) | (false, None) => path.to_path_buf(),
        (false, Some(cwd)) => cwd.join(path),
    };
    rikka_open::path_to_uri(&absolute)
}

fn has_scheme(argument: &str) -> bool {
    let Some((scheme, _)) = argument.split_once(':') else {
        return false;
    };
    scheme.len() > 1
        && scheme.starts_with(|letter: char| letter.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|letter| letter.is_ascii_alphanumeric() || matches!(letter, '+' | '-' | '.'))
}
