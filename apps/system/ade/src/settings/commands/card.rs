use ade_command::Command;
use rikka_app::prelude::*;

use crate::settings::rows::entry;

pub(crate) fn card(command: &Command) -> impl View {
    entry(
        format!("/{}", command.name()),
        command.scope(),
        command.path(),
    )
}
