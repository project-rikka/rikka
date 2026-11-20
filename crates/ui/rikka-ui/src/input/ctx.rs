use rikka_theme::Theme;
use rikka_types::{Rect, Size, Stylus};

use crate::input::{Accept, DragStart, DragWindow};
use crate::task::{Action, Handler, IntoHandler};
use crate::view::{Chain, State, StateAccess};
use crate::window::WindowRequest;

#[derive(Default)]
pub(crate) struct DispatchShared {
    pub(crate) requests: Vec<WindowRequest>,
    pub(crate) repaint: bool,
    pub(crate) view: bool,
    pub(crate) capture: CaptureRequest,
    pub(crate) focus: FocusRequest,
    pub(crate) pressed: Option<u64>,
    pub(crate) handled: bool,
    pub(crate) drag_start: Option<(u64, DragStart)>,
    pub(crate) drag_accept: Option<(u64, Accept)>,
    pub(crate) drag_window: Option<DragWindow>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) enum CaptureRequest {
    #[default]
    None,
    Grab(u64),
    Exclusive(u64),
    Lock(u64),
    Release,
}

#[derive(Debug, Default)]
pub(crate) enum FocusRequest {
    #[default]
    Undecided,
    Keep,
    Set(u64),
    Return(u64),
}

pub(crate) struct RawEventCtx<'a> {
    pub(crate) shared: &'a mut DispatchShared,
    pub(crate) root: &'a mut dyn State,
    pub(crate) chain: &'a Chain,
    pub(crate) instance: u64,
    pub(crate) bounds: Rect,
    pub(crate) room: Rect,
    pub(crate) content_size: Size,
    pub(crate) viewport: Size,
    pub(crate) theme: &'a crate::ambient::ThemeHandle,
    pub(crate) env: crate::view::Env,
    pub(crate) hovered: bool,
    pub(crate) captured: bool,
    pub(crate) presses: u32,
    pub(crate) focused: bool,
    pub(crate) marks: &'a std::sync::Arc<crate::runtime::Marks>,
    pub(crate) stylus: Option<&'a Stylus>,
}

pub struct EventCtx<'a> {
    pub(crate) raw: RawEventCtx<'a>,
}

impl std::fmt::Debug for EventCtx<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventCtx")
            .field("bounds", &self.raw.bounds)
            .field("hovered", &self.raw.hovered)
            .field("captured", &self.raw.captured)
            .field("focused", &self.raw.focused)
            .finish_non_exhaustive()
    }
}

impl EventCtx<'_> {
    pub fn bounds(&self) -> Rect {
        self.raw.bounds
    }

    pub fn room(&self) -> Rect {
        self.raw.room
    }

    pub fn content_size(&self) -> Size {
        self.raw.content_size
    }

    pub fn viewport(&self) -> Size {
        self.raw.viewport
    }

    pub fn theme(&self) -> &Theme {
        self.raw.theme
    }

    pub fn hovered(&self) -> bool {
        self.raw.hovered
    }

    pub fn captured(&self) -> bool {
        self.raw.captured
    }

    pub fn presses(&self) -> u32 {
        self.raw.presses
    }

    pub fn focused(&self) -> bool {
        self.raw.focused
    }

    pub fn stylus(&self) -> Option<&Stylus> {
        self.raw.stylus
    }

    pub fn request_focus(&mut self) {
        if !matches!(self.raw.shared.focus, FocusRequest::Set(_)) {
            self.raw.shared.focus = FocusRequest::Set(self.raw.instance);
        }
    }

    pub fn keep_focus(&mut self) {
        if matches!(self.raw.shared.focus, FocusRequest::Undecided) {
            self.raw.shared.focus = FocusRequest::Keep;
        }
    }

    pub fn return_focus(&mut self) {
        self.raw.shared.focus = FocusRequest::Return(self.raw.instance);
    }

    pub fn call<Args: 'static, R: 'static>(
        &mut self,
        handler: &Handler<Args, R>,
        args: Args,
    ) -> Option<R> {
        let mut reach = StateAccess::new(&mut *self.raw.root, self.raw.chain);
        handler.call(&mut reach, args)
    }

    pub fn act<Args: 'static>(&mut self, action: &Action<Args>, args: Args) {
        self.answer(action, args);
    }

    pub fn answer<Args: 'static, R: 'static>(
        &mut self,
        action: &Action<Args, R>,
        args: Args,
    ) -> Option<R> {
        self.call(action.handler()?, args)
    }

    pub fn run<R: 'static, Marker>(
        &mut self,
        action: impl IntoHandler<(), R, Marker>,
    ) -> Option<R> {
        self.call(&action.into_handler(), ())
    }

    pub fn request_window(&mut self, request: WindowRequest) {
        self.raw.shared.requests.push(request);
    }

    pub fn handle(&self) -> crate::runtime::NodeHandle {
        crate::runtime::NodeHandle::new(self.raw.instance, self.raw.marks)
    }

    pub fn request_layout(&mut self) {
        self.handle().relayout();
    }

    pub fn request_paint(&mut self) {
        self.raw.shared.repaint = true;
    }

    pub fn request_view(&mut self) {
        self.raw.shared.view = true;
    }

    pub fn capture_pointer(&mut self) {
        self.raw.shared.capture = CaptureRequest::Grab(self.raw.instance);
    }

    pub fn capture_pointer_exclusively(&mut self) {
        self.raw.shared.capture = CaptureRequest::Exclusive(self.raw.instance);
    }

    pub fn lock_pointer(&mut self) {
        self.raw.shared.capture = CaptureRequest::Lock(self.raw.instance);
    }

    pub fn release_pointer(&mut self) {
        self.raw.shared.capture = CaptureRequest::Release;
    }

    pub fn set_handled(&mut self) {
        self.raw.shared.handled = true;
    }

    pub fn start_drag(&mut self, drag: DragStart) {
        self.raw.shared.drag_start = Some((self.raw.instance, drag));
    }

    pub fn accept_drag(&mut self, accept: Accept) {
        self.raw.shared.drag_accept = Some((self.raw.instance, accept));
    }

    pub fn carry_window(&mut self, window: DragWindow) {
        self.raw.shared.drag_window = Some(window);
    }
}
