use clap::CommandFactory as _;
use cli_command::Cli;

#[test]
fn grammar_is_valid() {
    Cli::command().debug_assert();
}
