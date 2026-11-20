use std::rc::Rc;

use rikka_app::{LaunchRequest, Sender, WindowShape};
use rikka_ui::{Command, Element, Handler, Key, Modifiers, WindowRef};

use crate::abi;
use crate::ffi;
use crate::handlers::{Callback, action, fixed_flag};
use crate::view::View;
use crate::window::Model;

#[derive(Debug)]
pub(crate) struct WindowBuilder {
    view: Callback<ffi::ViewFn>,
    title: String,
    size: Option<(f32, f32)>,
    min_size: Option<(f32, f32)>,
    resizable: bool,
    maximized: bool,
    parent: Option<WindowRef>,
    modal: bool,
    start: Option<Handler>,
    close: Option<Handler>,
    focus: Option<Rc<Callback<ffi::BoolFn>>>,
    shape: Option<Callback<ffi::ShapeFn>>,
    launch: Option<Callback<ffi::LaunchFn>>,
    shortcuts: Vec<(Key, Modifiers, Handler)>,
    commands: Vec<(Command, Handler)>,
}

impl WindowBuilder {
    pub(crate) fn new(view: Callback<ffi::ViewFn>) -> Self {
        Self {
            view,
            title: String::new(),
            size: None,
            min_size: None,
            resizable: true,
            maximized: false,
            parent: None,
            modal: false,
            start: None,
            close: None,
            focus: None,
            shape: None,
            launch: None,
            shortcuts: Vec::new(),
            commands: Vec::new(),
        }
    }

    pub(crate) fn set_title(&mut self, title: &str) {
        title.clone_into(&mut self.title);
    }

    pub(crate) fn set_size(&mut self, width: f32, height: f32) {
        self.size = Some(positive_size(width, height));
    }

    pub(crate) fn set_min_size(&mut self, width: f32, height: f32) {
        self.min_size = Some(positive_size(width, height));
    }

    pub(crate) fn set_resizable(&mut self, resizable: bool) {
        self.resizable = resizable;
    }

    pub(crate) fn set_maximized(&mut self, maximized: bool) {
        self.maximized = maximized;
    }

    pub(crate) fn set_parent(&mut self, parent: WindowRef) {
        self.parent = Some(parent);
    }

    pub(crate) fn set_modal(&mut self, modal: bool) {
        self.modal = modal;
    }

    pub(crate) fn on_start(&mut self, callback: Callback<ffi::ActionFn>) {
        self.start = Some(action(callback));
    }

    pub(crate) fn on_close_request(&mut self, callback: Callback<ffi::ActionFn>) {
        self.close = Some(action(callback));
    }

    pub(crate) fn on_focus_change(&mut self, callback: Callback<ffi::BoolFn>) {
        self.focus = Some(Rc::new(callback));
    }

    pub(crate) fn on_shape(&mut self, callback: Callback<ffi::ShapeFn>) {
        self.shape = Some(callback);
    }

    pub(crate) fn on_launch(&mut self, callback: Callback<ffi::LaunchFn>) {
        self.launch = Some(callback);
    }

    pub(crate) fn add_shortcut(&mut self, key: Key, modifiers: Modifiers, handler: Handler) {
        self.shortcuts.push((key, modifiers, handler));
    }

    pub(crate) fn add_command(&mut self, command: Command, handler: Handler) {
        self.commands.push((command, handler));
    }

    pub(crate) fn into_window(self) -> rikka_app::Window<Model> {
        let Self {
            view,
            title,
            size,
            min_size,
            resizable,
            maximized,
            parent,
            modal,
            start,
            close,
            focus,
            shape,
            launch,
            shortcuts,
            commands,
        } = self;
        let initial = title.clone();
        let mut window = rikka_app::Window::new(
            move |sender: Sender<Model>| Model::new(sender, title),
            move |model: &Model| render(&view, model),
        )
        .title(initial)
        .title_from(|model: &Model| model.window.title())
        .resizable(resizable)
        .maximized(maximized)
        .modal(modal);
        if let Some((width, height)) = size {
            window = window.size(width, height);
        }
        if let Some((width, height)) = min_size {
            window = window.min_size(width, height);
        }
        if let Some(parent) = parent {
            window = window.parent(parent);
        }
        if let Some(start) = start {
            window = window.on_start(start);
        }
        if let Some(close) = close {
            window = window.on_close_request(close);
        }
        if let Some(focus) = focus {
            window = window
                .on_focus(fixed_flag(Rc::clone(&focus), true))
                .on_blur(fixed_flag(focus, false));
        }
        if let Some(shape) = shape {
            window = window.on_shape(move |model: &mut Model, shaped: WindowShape| {
                shape.call(
                    &mut model.context(),
                    shaped.size.width,
                    shaped.size.height,
                    shaped.maximized,
                    shaped.fullscreen,
                );
            });
        }
        if let Some(launch) = launch {
            window = window.on_open(move |model: &mut Model, request: LaunchRequest| {
                let arguments: Vec<String> = request
                    .arguments()
                    .iter()
                    .map(|argument| argument.to_string_lossy().into_owned())
                    .collect();
                let borrowed: Vec<&str> = arguments.iter().map(String::as_str).collect();
                launch.call(&mut model.context(), &borrowed);
            });
        }
        for (key, modifiers, handler) in shortcuts {
            window = window.on_chord(key, modifiers, handler);
        }
        for (command, handler) in commands {
            window = window.on_command(command, handler);
        }
        window
    }
}

fn positive_size(width: f32, height: f32) -> (f32, f32) {
    if !(width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0) {
        abi::violation(format!("{width} x {height} is not a positive size"));
    }
    (width, height)
}

fn render(view: &Callback<ffi::ViewFn>, model: &Model) -> Element {
    let _entry = abi::enter("rk_view_fn");
    let mut built = View::new();
    view.call(&mut built);
    built.finish_window(model.dialogs().elements())
}
