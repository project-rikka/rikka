mod chooser;
mod options;
mod results;
mod wire;

use std::process::ExitCode;

use rikka_portal_backend::Helper;

use crate::chooser::FileChooser;

const BUS_NAME: &str = "org.freedesktop.impl.portal.desktop.rikkafiles";

const DIALOG: &str = "rikka-file-dialog";

const DIALOG_ENV: &str = "RIKKA_FILE_DIALOG";

fn main() -> ExitCode {
    rikka_log::install("warn");
    let dialog = Helper::program(DIALOG_ENV, DIALOG);
    match rikka_portal_backend::serve(BUS_NAME, FileChooser::new(dialog)) {
        Ok(connection) => {
            connection.closed();
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("rikka-portal-files: cannot answer as {BUS_NAME}: {error}");
            ExitCode::FAILURE
        }
    }
}
