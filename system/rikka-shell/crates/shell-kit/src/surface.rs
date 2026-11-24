use std::rc::Rc;
use std::time::Duration;

use rikka_renderer::Buffering;
use rikka_types::{Color, Size};
use rikka_ui::{Element, Handler, IntoHandler, State, ViewFn};

use crate::extent::{Extent, Span};
use crate::presence_binding::PresenceBinding;
use crate::settings::{OutputTarget, SurfaceSettings};
use crate::{
    AimTarget, Aimed, Anchor, Background, Destination, ExclusiveZone, KeyboardMode, Layer, Margin,
    Opacity, Placement, PointerMode, Positioner, Presence, Reach, SessionLock,
};

type SurfaceView<S> = Rc<dyn Fn(&S) -> Element>;

pub struct SurfaceDesc<S: State> {
    pub(crate) settings: SurfaceSettings,
    pub(crate) source: Box<dyn Fn(Placement) -> S>,
    pub(crate) view: SurfaceView<S>,
    pub(crate) start: Option<Handler>,
    pub(crate) moved: Option<Handler<(Placement,)>>,
    pub(crate) leave: Option<Handler>,
}

impl<S: State> SurfaceDesc<S> {
    pub fn new(factory: impl Fn() -> S + 'static, view: impl for<'a> ViewFn<'a, S>) -> Self {
        Self::from_parts(Box::new(move |_| factory()), view)
    }

    pub fn per_output(
        factory: impl Fn(Placement) -> S + 'static,
        view: impl for<'a> ViewFn<'a, S>,
    ) -> Self {
        let mut desc = Self::from_parts(Box::new(factory), view);
        desc.settings.output = OutputTarget::All;
        desc
    }

    pub fn aimed(
        aimed: Aimed,
        factory: impl Fn(Placement, Option<AimTarget>) -> S + 'static,
        view: impl for<'a> ViewFn<'a, S>,
    ) -> Self {
        let aim_source = aimed.clone();
        let mut desc = Self::from_parts(
            Box::new(move |placement| {
                let aim = aim_source.target_on(&placement);
                factory(placement, aim)
            }),
            view,
        );
        desc.settings.output = OutputTarget::Aimed(aimed);
        desc
    }

    pub fn destined(
        destination: Destination,
        factory: impl Fn(Placement) -> S + 'static,
        view: impl for<'a> ViewFn<'a, S>,
    ) -> Self {
        let mut desc = Self::from_parts(Box::new(factory), view);
        desc.settings.output = OutputTarget::Destined(destination);
        desc
    }

    fn from_parts(source: Box<dyn Fn(Placement) -> S>, view: impl for<'a> ViewFn<'a, S>) -> Self {
        Self {
            settings: SurfaceSettings::new(),
            source,
            view: Rc::new(move |state| view.build(state)),
            start: None,
            moved: None,
            leave: None,
        }
    }

    pub fn on_start<M>(mut self, start: impl IntoHandler<(), (), M>) -> Self {
        self.start = Some(start.into_handler());
        self
    }

    pub fn on_moved<M>(mut self, moved: impl IntoHandler<(Placement,), (), M>) -> Self {
        self.moved = Some(moved.into_handler());
        self
    }

    pub fn on_leave<M>(mut self, leave: impl IntoHandler<(), (), M>) -> Self {
        self.leave = Some(leave.into_handler());
        self
    }

    pub fn presence(mut self, presence: Presence) -> Self {
        self.settings.presence_binding = Some(PresenceBinding::at_once(presence));
        self
    }

    pub fn animated(mut self, presence: Presence, exit: impl Into<Duration>) -> Self {
        self.settings.presence_binding = Some(PresenceBinding::playing(presence, exit.into()));
        self
    }

    pub fn after(mut self, presence: Presence) -> Self {
        self.settings.after.push(presence);
        self
    }

    pub fn wallpaper(self) -> Self {
        let mut desc = self.desktop();
        desc.settings.layer = Layer::Background;
        desc.settings.background = Background::Solid(Color::BLACK);
        desc.settings.buffering = Buffering::Double;
        desc
    }

    pub fn desktop(mut self) -> Self {
        self.settings.layer = Layer::Bottom;
        self.settings.extent = Extent::FILL;
        self.settings.exclusive = ExclusiveZone::Ignore;
        self
    }

    pub fn session_lock(mut self, lock: &SessionLock) -> Self {
        self.settings.presence_binding = Some(PresenceBinding::at_once(lock.presence()));
        self.settings.lock = Some(lock.clone());
        self.settings.output = OutputTarget::All;
        self.settings.extent = Extent::FILL;
        self.settings.exclusive = ExclusiveZone::Ignore;
        self.settings.keyboard = KeyboardMode::Exclusive;
        self.settings.background = Background::Solid(Color::BLACK);
        self
    }

    pub fn modal(self, presence: Presence, exit: impl Into<Duration>) -> Self {
        self.overlay()
            .keyboard(KeyboardMode::Exclusive)
            .animated(presence, exit)
    }

    pub fn overlay(mut self) -> Self {
        self.settings.layer = Layer::Overlay;
        self.settings.extent = Extent::FILL;
        self.settings.exclusive = ExclusiveZone::Ignore;
        self
    }

    pub fn keyboard(mut self, mode: KeyboardMode) -> Self {
        self.settings.keyboard = mode;
        self
    }

    pub fn pointer(mut self, mode: PointerMode) -> Self {
        self.settings.pointer = mode;
        self
    }

    pub fn reach(mut self, reach: Reach) -> Self {
        self.settings.reach = Some(reach);
        self
    }

    pub fn opacity(mut self, opacity: Opacity) -> Self {
        self.settings.opacity = Some(opacity);
        self
    }

    pub fn drives(mut self, opacity: Opacity) -> Self {
        self.settings.drives = Some(opacity);
        self
    }

    pub fn fitted(mut self) -> Self {
        self.settings.fits = true;
        self
    }

    pub fn positioned(mut self, positioner: Positioner) -> Self {
        self.settings.positioner = Some(positioner);
        self.settings.anchor = Anchor::TOP | Anchor::LEFT;
        self.settings.exclusive = ExclusiveZone::Ignore;
        self.settings.fits = true;
        self
    }

    pub fn scrim(mut self) -> Self {
        self.settings.is_scrim = true;
        self
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.settings.name = Some(name.into());
        self
    }

    pub fn layer(mut self, layer: Layer) -> Self {
        self.settings.layer = layer;
        self
    }

    pub fn anchor(mut self, anchor: impl Into<Anchor>) -> Self {
        self.settings.anchor = anchor.into();
        self
    }

    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.settings.extent = Extent::fixed(Size::new(width, height));
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.settings.extent.width = Span::Fixed(width);
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.settings.extent.height = Span::Fixed(height);
        self
    }

    pub fn margin(mut self, margin: impl Into<Margin>) -> Self {
        self.settings.margin = margin.into();
        self
    }

    pub fn exclusive_zone(mut self, zone: ExclusiveZone) -> Self {
        self.settings.exclusive = zone;
        self
    }
}

impl<S: State> std::fmt::Debug for SurfaceDesc<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SurfaceDesc")
            .field("settings", &self.settings)
            .field("state", &std::any::type_name::<S>())
            .finish_non_exhaustive()
    }
}
