mod dialog;
mod view;

use std::sync::Arc;

use files_dialog::{Ask, Kind, Outcome, Reply};
use rikka_app::prelude::*;
use rikka_mime::Database;

use crate::chooser::dialog::Dialog;
use crate::wording;

pub fn choose() -> rikka_app::Result<()> {
    let reply = match Reply::claim() {
        Ok(reply) => reply,
        Err(error) => {
            eprintln!("rikka-file-dialog: failed to claim reply channel: {error}");
            std::process::exit(2);
        }
    };

    let loading = std::thread::Builder::new()
        .name("rikka-file-dialog".to_owned())
        .spawn(Database::load);

    let ask = match Ask::read(std::io::stdin().lock()) {
        Ok(ask) => ask,
        Err(error) => {
            eprintln!("rikka-file-dialog: {error}");
            std::process::exit(2);
        }
    };

    let title = if ask.title.is_empty() {
        Label::translated(match ask.kind {
            Kind::Open if ask.directory => "files.chooser.title-folder",
            Kind::Open => "files.chooser.title-open",
            Kind::Save => "files.chooser.title-save",
            Kind::SaveInto => "files.chooser.title-save-into",
        })
    } else {
        Label::from(ask.title.as_str())
    };

    let parent = ask.parent.clone();
    let modal = ask.modal;
    let answering = reply.clone();
    let window = view::bind(Window::new(
        move |sender: Sender<Dialog>| {
            let mime = loading.map_or_else(
                |_| Database::load(),
                |loading| loading.join().unwrap_or_else(|_| Database::load()),
            );
            Dialog::new(sender, ask, Arc::new(mime), answering)
        },
        view::view,
    ))
    .title(title)
    .on_start(Dialog::start)
    .size(880.0, 580.0)
    .min_size(640.0, 400.0);
    let window = match parent {
        Some(parent) => window.parent(parent).modal(modal),
        None => window,
    };
    App::new("rikka-file-dialog")
        .i18n(wording::catalogs())
        .window(window)
        .run()?;

    reply.answer(&Outcome::Cancelled)
}
