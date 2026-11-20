use rikka_sync::{Mutex, MutexGuard};
use std::cell::OnceCell;
use std::sync::Arc;

use files_core::Location;
use files_ipc::{Request, Reveal};
use rikka_app::prelude::*;

use crate::tab::Tab;
use crate::window::{self, WindowState};
use crate::wording;

thread_local! {
    static DESKTOP: OnceCell<Option<files_ipc::Service>> = const { OnceCell::new() };
}

pub fn manage() -> rikka_app::Result<()> {
    let windows = Windows::default();
    App::new("rikka-files")
        .i18n(wording::catalogs())
        .windows(move |open: LaunchRequest| opened(&windows, &open))
        .run()
}

fn opened(windows: &Windows, open: &LaunchRequest) -> Window<WindowState> {
    let (start, selected) = Reveal::classify(open.uris())
        .and_then(|reveal| reveal.destination())
        .unwrap_or_else(|| (crate::listing::home(), Vec::new()));
    window_at(windows.clone(), start, selected)
}

pub(crate) fn window_at(
    windows: Windows,
    start: Location,
    selected: Vec<Location>,
) -> Window<WindowState> {
    described(Window::new(
        move |sender: Sender<WindowState>| {
            register(&windows, &sender);
            WindowState::new(sender, windows, start, selected)
        },
        window::view,
    ))
}

pub(crate) fn window_with_tab(windows: Windows, tab: Tab) -> Window<WindowState> {
    described(Window::new(
        move |sender: Sender<WindowState>| {
            register(&windows, &sender);
            WindowState::with_tab(sender, windows, tab)
        },
        window::view,
    ))
}

fn described(window: Window<WindowState>) -> Window<WindowState> {
    window::keys::bind(window)
        .title(tr!("files.title"))
        .size(1080.0, 700.0)
        .min_size(720.0, 420.0)
        .on_start(WindowState::start)
        .caption(|state: &WindowState| Some(caption().fill(window::strip(state)?).into()))
}

fn register(windows: &Windows, sender: &Sender<WindowState>) {
    windows.register(sender.clone());
    serve_file_manager(windows);
}

fn serve_file_manager(windows: &Windows) {
    DESKTOP.with(|desktop| {
        desktop.get_or_init(|| {
            let windows = windows.clone();
            files_ipc::start(Arc::new(move |request: Request| windows.deliver(&request)))
                .inspect_err(
                    |error| tracing::warn!(%error, "cannot serve org.freedesktop.FileManager1"),
                )
                .ok()
        });
    });
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Windows(Arc<Mutex<Vec<Sender<WindowState>>>>);

impl Windows {
    fn register(&self, sender: Sender<WindowState>) {
        let mut windows = self.lock();
        windows.retain(Sender::is_alive);
        windows.push(sender);
    }

    fn deliver(&self, request: &Request) {
        let mut windows = self.lock();
        while let Some(sender) = windows.last() {
            let request = request.clone();
            if sender
                .send(move |window: &mut WindowState| window.requested(request))
                .is_ok()
            {
                return;
            }
            windows.pop();
        }
    }

    fn lock(&self) -> MutexGuard<'_, Vec<Sender<WindowState>>> {
        self.0.lock()
    }
}
