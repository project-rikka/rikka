mod sources;
mod view;
mod window;

use std::process::ExitCode;

use rikka_app::{App, Sender, Window};
use rikka_portal::Reply;
use rikka_ui::tr;

use crate::picker::window::Picker;
use crate::protocol::{Outcome, PickerRequest};

pub(crate) const APP_ID: &str = "rikka-cast-picker";

pub(crate) fn run() -> ExitCode {
    let reply = match Reply::claim() {
        Ok(reply) => reply,
        Err(error) => {
            eprintln!("rikka-cast-picker: failed to claim reply channel: {error}");
            return ExitCode::from(2);
        }
    };

    let request = match PickerRequest::read(std::io::stdin().lock()) {
        Ok(request) => request,
        Err(error) => {
            eprintln!("rikka-cast-picker: {error}");
            return ExitCode::from(2);
        }
    };

    let answering = reply.clone();
    let result = App::new(APP_ID)
        .i18n(rikka_i18n::embedded!())
        .window(
            Window::new(
                move |sender: Sender<Picker>| Picker::new(sender, request, answering),
                view::view,
            )
            .title(tr!("cast.title"))
            .size(940.0, 660.0)
            .min_size(660.0, 460.0)
            .on_start(|picker: &mut Picker| picker.load_sources()),
        )
        .run();
    if let Err(error) = result {
        eprintln!("rikka-cast-picker: {error}");
        return ExitCode::FAILURE;
    }

    reply.answer(&Outcome::Cancelled)
}
