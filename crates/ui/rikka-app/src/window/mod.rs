mod bindings;
mod chrome;
mod closing;
mod parent;
mod requests;
mod shape;
mod window_title;

use std::any::type_name;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rikka_instance::LaunchRequest;
use rikka_theme::Theme;
use rikka_types::{Key, Modifiers, Size};
use rikka_ui::{Command, Element, Handler, IntoHandler, Label, State, TypedUi, Ui, ViewFn};
use rikka_widgets::CaptionBar;

use bindings::Bindings;
#[cfg(unix)]
pub(crate) use chrome::set_client_side;
pub(crate) use chrome::{Decorated, set_chrome};
pub(crate) use closing::Closing;
pub use parent::WindowParent;
pub use requests::open_window;
pub(crate) use requests::{accept_window_requests, take_requested_windows};
pub(crate) use shape::Shaping;
pub use shape::WindowShape;
pub(crate) use window_title::WindowTitle;

use crate::sender::{Route, Sender};

pub(crate) struct BuiltWindow {
    pub(crate) theme: Theme,
    pub(crate) client_side: bool,
    pub(crate) route: Route,
    pub(crate) alive: Arc<AtomicBool>,
}

pub trait Factory<S: State, Marker>: 'static {
    fn make(self, sender: Sender<S>) -> S;
}

#[doc(hidden)]
#[derive(Debug)]
pub struct WithoutSender;

#[doc(hidden)]
#[derive(Debug)]
pub struct WithSender;

impl<S: State, F: FnOnce() -> S + 'static> Factory<S, WithoutSender> for F {
    fn make(self, _sender: Sender<S>) -> S {
        self()
    }
}

impl<S: State, F: FnOnce(Sender<S>) -> S + 'static> Factory<S, WithSender> for F {
    fn make(self, sender: Sender<S>) -> S {
        self(sender)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RequestedGeometry {
    pub(crate) size: Size,
    pub(crate) min_size: Option<Size>,
    pub(crate) resizable: bool,
}

#[derive(Debug)]
pub(crate) struct WindowConfig {
    pub(crate) title: Label,
    pub(crate) size: Size,
    pub(crate) min_size: Option<Size>,
    pub(crate) resizable: bool,
    pub(crate) maximized: bool,
    pub(crate) server_decorations: bool,
    pub(crate) transparent: bool,
    pub(crate) parent: Option<WindowParent>,
    pub(crate) modal: bool,
    #[cfg(unix)]
    pub(crate) token: Option<String>,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: Label::default(),
            size: Size::new(800.0, 600.0),
            min_size: None,
            resizable: true,
            maximized: false,
            server_decorations: false,
            transparent: false,
            parent: None,
            modal: false,
            #[cfg(unix)]
            token: None,
        }
    }
}

type MakeFn<S> = Box<dyn FnOnce(Sender<S>) -> S>;
type ViewFor<S> = Box<dyn Fn(&S) -> Element>;
type CaptionFn<S> = Box<dyn Fn(&S) -> Option<CaptionBar>>;
type ThemeFn<S> = Box<dyn Fn(&S) -> Option<Theme>>;
type TitleFn<S> = Box<dyn Fn(&S) -> Label>;

#[must_use]
pub struct Window<S: State> {
    look: WindowConfig,
    start: Option<Handler>,
    focus: Option<Handler>,
    blur: Option<Handler>,
    language: Option<Handler>,
    close: Option<Handler>,
    shape: Option<Handler<(WindowShape,)>>,
    open_handler: Option<Handler<(LaunchRequest,)>>,
    make: MakeFn<S>,
    view: ViewFor<S>,
    caption: Option<CaptionFn<S>>,
    theme: Option<ThemeFn<S>>,
    title: Option<TitleFn<S>>,
    bindings: Bindings,
}

impl<S: State> Window<S> {
    pub fn new<M>(factory: impl Factory<S, M>, view: impl for<'a> ViewFn<'a, S>) -> Self {
        Self {
            look: WindowConfig::default(),
            start: None,
            focus: None,
            blur: None,
            language: None,
            close: None,
            shape: None,
            open_handler: None,
            make: Box::new(move |sender| factory.make(sender)),
            view: Box::new(move |state| view.build(state)),
            caption: None,
            theme: None,
            title: None,
            bindings: Bindings::default(),
        }
    }

    pub fn on_command<M>(mut self, command: Command, handler: impl IntoHandler<(), (), M>) -> Self {
        self.bindings.command(command, handler.into_handler());
        self
    }

    pub fn on_chord<M>(
        mut self,
        key: Key,
        modifiers: Modifiers,
        handler: impl IntoHandler<(), (), M>,
    ) -> Self {
        self.bindings.chord(key, modifiers, handler.into_handler());
        self
    }

    pub fn title(mut self, title: impl Into<Label>) -> Self {
        self.look.title = title.into();
        self
    }

    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.look.size = Size::new(width, height);
        self
    }

    pub fn min_size(mut self, width: f32, height: f32) -> Self {
        self.look.min_size = Some(Size::new(width, height));
        self
    }

    pub fn resizable(mut self, resizable: bool) -> Self {
        self.look.resizable = resizable;
        self
    }

    pub fn maximized(mut self, maximized: bool) -> Self {
        self.look.maximized = maximized;
        self
    }

    pub fn server_decorations(mut self, server_decorations: bool) -> Self {
        self.look.server_decorations = server_decorations;
        self
    }

    pub fn transparent(mut self, transparent: bool) -> Self {
        self.look.transparent = transparent;
        self
    }

    pub fn parent(mut self, parent: impl Into<WindowParent>) -> Self {
        self.look.parent = Some(parent.into());
        self
    }

    pub fn modal(mut self, modal: bool) -> Self {
        self.look.modal = modal;
        self
    }

    pub fn on_start<M>(mut self, start: impl IntoHandler<(), (), M>) -> Self {
        self.start = Some(start.into_handler());
        self
    }

    pub fn on_open<M>(mut self, handler: impl IntoHandler<(LaunchRequest,), (), M>) -> Self {
        self.open_handler = Some(handler.into_handler());
        self
    }

    pub fn on_focus<M>(mut self, focus: impl IntoHandler<(), (), M>) -> Self {
        self.focus = Some(focus.into_handler());
        self
    }

    pub fn on_blur<M>(mut self, blur: impl IntoHandler<(), (), M>) -> Self {
        self.blur = Some(blur.into_handler());
        self
    }

    pub fn on_language<M>(mut self, language: impl IntoHandler<(), (), M>) -> Self {
        self.language = Some(language.into_handler());
        self
    }

    pub fn on_close_request<M>(mut self, close: impl IntoHandler<(), (), M>) -> Self {
        self.close = Some(close.into_handler());
        self
    }

    pub fn on_shape<M>(mut self, shape: impl IntoHandler<(WindowShape,), (), M>) -> Self {
        self.shape = Some(shape.into_handler());
        self
    }

    pub fn caption(mut self, caption: impl Fn(&S) -> Option<CaptionBar> + 'static) -> Self {
        self.caption = Some(Box::new(caption));
        self
    }

    pub fn theme(mut self, theme: impl Fn(&S) -> Option<Theme> + 'static) -> Self {
        self.theme = Some(Box::new(theme));
        self
    }

    pub fn title_from<T: Into<Label>>(mut self, title: impl Fn(&S) -> T + 'static) -> Self {
        self.title = Some(Box::new(move |state| title(state).into()));
        self
    }
}

impl<S: State> std::fmt::Debug for Window<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Window")
            .field("state", &type_name::<S>())
            .field("look", &self.look)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Default)]
pub(crate) struct WindowHandlers {
    pub(crate) focus: Option<Handler>,
    pub(crate) blur: Option<Handler>,
    #[cfg(unix)]
    pub(crate) language: Option<Handler>,
    pub(crate) shape: Option<Handler<(WindowShape,)>>,
    pub(crate) open_handler: Option<Handler<(LaunchRequest,)>>,
    pub(crate) closing: Closing,
    pub(crate) window_title: WindowTitle,
}

#[must_use]
pub struct Opening {
    pub(crate) look: WindowConfig,
    pub(crate) hooks: WindowHandlers,
    build: Box<dyn FnOnce(BuiltWindow) -> Ui>,
}

impl Opening {
    pub(crate) fn into_ui(self, built: BuiltWindow) -> Ui {
        (self.build)(built)
    }
}

impl std::fmt::Debug for Opening {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Opening")
            .field("look", &self.look)
            .finish_non_exhaustive()
    }
}

pub(crate) struct Content<S: State> {
    start: Option<Handler>,
    make: MakeFn<S>,
    view: ViewFor<S>,
    caption: Option<CaptionFn<S>>,
    theme: Option<ThemeFn<S>>,
    title: Option<TitleFn<S>>,
    window_title: WindowTitle,
    bindings: Bindings,
}

impl<S: State> Window<S> {
    pub(crate) fn into_parts(self) -> (WindowConfig, WindowHandlers, Content<S>) {
        let Window {
            look,
            start,
            focus,
            blur,
            language,
            close,
            shape,
            open_handler,
            make,
            view,
            caption,
            theme,
            title,
            bindings,
        } = self;
        #[cfg(windows)]
        drop(language);
        let window_title = WindowTitle::default();
        let content = Content {
            start,
            make,
            view,
            caption,
            theme,
            title,
            window_title: window_title.clone(),
            bindings,
        };
        let hooks = WindowHandlers {
            focus,
            blur,
            #[cfg(unix)]
            language,
            shape,
            open_handler,
            closing: Closing::hooked(close),
            window_title,
        };
        (look, hooks, content)
    }
}

impl<S: State> Content<S> {
    pub(crate) fn into_ui(
        self,
        sender: Sender<S>,
        theme: Theme,
        client_side: bool,
    ) -> TypedUi<Decorated<S>> {
        let Content {
            start,
            make,
            view,
            caption,
            theme: theme_of,
            title,
            window_title,
            bindings,
        } = self;
        let state = make(sender);
        let mut ui = TypedUi::new(
            Decorated::new(state, client_side),
            move |decorated: &Decorated<S>| {
                if let Some(title) = &title {
                    window_title.set(title(decorated.inner()));
                }
                bindings.around(decorated.frame(view(decorated.inner()), |state| {
                    caption.as_ref().and_then(|caption| caption(state))
                }))
            },
            theme,
        );
        if let Some(theme_of) = theme_of {
            ui = ui.themed(move |decorated| theme_of(decorated.inner()));
        }
        if let Some(start) = start {
            ui.start(start);
        }
        ui
    }
}

impl<S: State> From<Window<S>> for Opening {
    fn from(window: Window<S>) -> Self {
        let (look, hooks, content) = window.into_parts();
        Opening {
            look,
            hooks,
            build: Box::new(move |built: BuiltWindow| {
                let sender = Sender::new(built.route, built.alive);
                content
                    .into_ui(sender, built.theme, built.client_side)
                    .into()
            }),
        }
    }
}

pub trait IntoWindows {
    fn into_windows(self) -> Vec<Opening>;
}

impl IntoWindows for Opening {
    fn into_windows(self) -> Vec<Opening> {
        vec![self]
    }
}

impl<S: State> IntoWindows for Window<S> {
    fn into_windows(self) -> Vec<Opening> {
        vec![self.into()]
    }
}

impl IntoWindows for Vec<Opening> {
    fn into_windows(self) -> Vec<Opening> {
        self
    }
}

impl<S: State> IntoWindows for Vec<Window<S>> {
    fn into_windows(self) -> Vec<Opening> {
        self.into_iter().map(Into::into).collect()
    }
}
