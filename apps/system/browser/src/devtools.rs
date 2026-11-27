use rikka_app::prelude::*;
use rikka_web::{ForeignWindow, foreign_view};

const WIDTH: f32 = 1024.0;
const HEIGHT: f32 = 720.0;

#[derive(State)]
pub(crate) struct DevTools {
    #[untracked]
    window: ForeignWindow,
}

impl DevTools {
    fn title(&self) -> String {
        self.window
            .title()
            .unwrap_or_else(|| tr!("browser.devtools.title").to_string())
    }

    fn close(&mut self) {
        self.window.close();
        request_window(WindowRequest::Close);
    }
}

impl std::fmt::Debug for DevTools {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DevTools")
            .field("window", &self.window)
            .finish()
    }
}

pub(crate) fn window(foreign: ForeignWindow) -> Window<DevTools> {
    Window::new(
        move |sender: Sender<DevTools>| {
            foreign.on_closed(move || {
                let closing = sender.send(|_: &mut DevTools| request_window(WindowRequest::Close));
                if closing.is_err() {
                    tracing::debug!("DevTools window already closed");
                }
            });
            DevTools { window: foreign }
        },
        view,
    )
    .title(tr!("browser.devtools.title"))
    .title_from(DevTools::title)
    .size(WIDTH, HEIGHT)
    .on_close_request(DevTools::close)
}

fn view(devtools: &DevTools) -> Element {
    foreign_view(&devtools.window).fill().any()
}
