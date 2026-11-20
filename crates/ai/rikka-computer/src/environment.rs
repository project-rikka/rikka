use std::ffi::{OsStr, OsString};
use std::process::Command;

use rikka_session::environment::SESSION_VARIABLES;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Environment {
    variables: Vec<(OsString, OsString)>,
}

impl Environment {
    pub(crate) fn new(variables: Vec<(OsString, OsString)>) -> Environment {
        Environment { variables }
    }

    pub fn apply(&self, command: &mut Command) {
        for name in SESSION_VARIABLES {
            if !self.defines(OsStr::new(name)) {
                command.env_remove(name);
            }
        }
        command.envs(self.variables.iter().cloned());
    }

    fn defines(&self, name: &OsStr) -> bool {
        self.variables.iter().any(|(defined, _)| defined == name)
    }
}
