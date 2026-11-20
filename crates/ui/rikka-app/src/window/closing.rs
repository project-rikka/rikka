use rikka_ui::{Handler, Ui, WindowRequest};

#[derive(Clone, Default)]
pub(crate) struct Closing {
    hook: Option<Handler>,
}

impl Closing {
    pub(crate) fn hooked(hook: Option<Handler>) -> Self {
        Self { hook }
    }

    pub(crate) fn ask(&self, ui: &mut Ui) -> bool {
        match &self.hook {
            Some(hook) => {
                ui.start(hook.clone());
                false
            }
            None => true,
        }
    }

    pub(crate) fn serve(&self, ui: &mut Ui, mut other: impl FnMut(WindowRequest)) -> bool {
        let mut closes = false;
        let mut close_requested = false;
        for request in ui.take_window_requests() {
            match request {
                WindowRequest::Close => closes = true,
                WindowRequest::AskToClose => close_requested = true,
                request => other(request),
            }
        }
        if closes || !close_requested {
            return closes;
        }
        if self.ask(ui) {
            return true;
        }
        ui.take_window_requests()
            .into_iter()
            .fold(false, |closes, request| match request {
                WindowRequest::Close => true,
                WindowRequest::AskToClose => closes,
                request => {
                    other(request);
                    closes
                }
            })
    }
}

impl std::fmt::Debug for Closing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Closing")
            .field("hooked", &self.hook.is_some())
            .finish()
    }
}
