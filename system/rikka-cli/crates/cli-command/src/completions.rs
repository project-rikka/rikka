use std::io::Write;

use clap::CommandFactory;
use clap_complete::Shell;

use crate::cli::Cli;

pub fn write_completions(shell: Shell, out: &mut dyn Write) {
    let mut command = Cli::command();
    let name = command.get_name().to_owned();
    clap_complete::generate(shell, &mut command, name, out);
}
