mod builder;
mod dialogs;
mod listener;

use rikka_sync::Mutex;
use std::rc::Rc;
use std::sync::Arc;

use rikka_app::{SendError, Sender};
use rikka_ui::{Element, State, WindowRef};

use crate::abi::{self, AnyThread, Closure};
use crate::context::Context;
use crate::ffi;
use crate::handlers::Callback;
use crate::view::View;

pub(crate) use builder::WindowBuilder;
pub(crate) use dialogs::Dialogs;
pub(crate) use listener::Listener;

#[derive(Debug)]
pub(crate) struct Window {
    sender: Sender<Model>,
    title: Mutex<String>,
}

impl Window {
    pub(crate) fn invalidate(&self) {
        match self.sender.send(|_: &mut Model| {}) {
            Ok(()) | Err(SendError) => {}
        }
    }

    pub(crate) fn post(&self, callback: Closure<ffi::ActionFn, AnyThread>) -> ffi::Status {
        let sent = self
            .sender
            .send(move |model: &mut Model| callback.call(&mut model.context()));
        match sent {
            Ok(()) => ffi::Status::Ok,
            Err(SendError) => ffi::Status::Closed,
        }
    }

    pub(crate) fn is_open(&self) -> bool {
        self.sender.is_alive()
    }

    pub(crate) fn set_title(&self, title: &str) {
        title.clone_into(&mut self.title.lock());
        self.invalidate();
    }

    pub(crate) fn title(&self) -> String {
        self.title.lock().clone()
    }

    pub(crate) fn reference(&self) -> WindowRef {
        self.sender.window()
    }

    pub(crate) fn listener(&self, callback: Closure<ffi::ActionFn, AnyThread>) -> Listener {
        Listener::new(self.sender.clone(), callback)
    }
}

#[derive(State)]
pub(crate) struct Model {
    window: Arc<Window>,
    dialogs: Rc<Dialogs>,
}

impl Model {
    pub(crate) fn new(sender: Sender<Model>, title: String) -> Self {
        Self {
            window: Arc::new(Window {
                sender,
                title: Mutex::new(title),
            }),
            dialogs: Rc::default(),
        }
    }

    pub(crate) fn context(&self) -> Context {
        Context::new(Arc::clone(&self.window), Rc::clone(&self.dialogs))
    }

    pub(crate) fn window(&self) -> &Arc<Window> {
        &self.window
    }

    pub(crate) fn render(&self, view: &Callback<ffi::ViewFn>) -> Element {
        let _entry = abi::enter("rk_view_fn");
        let mut built = View::new();
        view.call(&mut built);
        built.finish_window(self.dialogs.elements())
    }
}
