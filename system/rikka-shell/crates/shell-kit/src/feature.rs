use crate::Shell;

pub trait Feature: Sized {
    fn install(self, shell: Shell) -> Shell;
}
