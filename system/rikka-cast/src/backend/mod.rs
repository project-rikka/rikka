mod cast;
mod options;
mod restore;
mod results;
mod screencast;
mod session;

use std::process::ExitCode;

use rikka_portal_backend::Helper;

use crate::backend::screencast::ScreenCast;

const BUS_NAME: &str = "org.freedesktop.impl.portal.desktop.rikkacast";

pub(crate) fn run() -> ExitCode {
    let picker = Helper::current_exe(crate::PICKER);
    match rikka_portal_backend::serve(BUS_NAME, ScreenCast::new(picker)) {
        Ok(connection) => {
            connection.closed();
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("rikka-portal-cast: cannot answer as {BUS_NAME}: {error}");
            ExitCode::FAILURE
        }
    }
}
