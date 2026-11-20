mod ghost;
mod wire;

use std::time::{Duration, Instant};

use rikka_types::Point;
use rikka_ui::{
    Accept, DragAction, DragEnd, DragIcon, DragOffer, DragStart, DragWindow, Event, Offer,
};
use rikka_wayland_client::wl_surface::WlSurface;

use crate::tasks::WindowId;
use crate::wayland::data_device::{self, Delivery, DragEvent, End, Icon};

use crate::wayland::drag::ghost::Ghost;
use crate::wayland::drag::wire::{Local, Wire};
use crate::wayland::hosting::{Hosting, Windows};
use crate::wayland::session::Session;

const PICTURE_POLL: Duration = Duration::from_millis(16);

#[derive(Default)]
pub(crate) struct Dragging {
    active: Option<ActiveDrag>,
    hovering: Option<Hovering>,
}

struct ActiveDrag {
    origin: WindowId,
    offer: Offer,
    announced: bool,
    ghost: Option<Ghost>,
    moves_windows: bool,
    within: Option<bool>,
    opening: Option<Point>,
    following: Option<Following>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Following {
    Opened(WindowId),
    Returned(WindowId),
    Origin,
}

impl Following {
    fn detached(self) -> Option<WindowId> {
        match self {
            Self::Opened(window) | Self::Returned(window) => Some(window),
            Self::Origin => None,
        }
    }
}

struct Hovering {
    window: WindowId,
    offer: DragOffer,
    position: Point,
    accepted: Option<Accept>,
}

impl Dragging {
    pub(crate) fn is_active(&self) -> bool {
        self.active.is_some()
    }

    pub(crate) fn begin_from(&mut self, windows: &mut Windows<'_>, id: WindowId) {
        if let Some(start) = windows
            .hosting
            .get_mut(&id)
            .and_then(|hosted| hosted.ui.take_drag_start())
            && (self.active.is_some() || !self.take_up(windows, id, start))
        {
            windows.dispatch_event(id, Event::DragEnded(DragEnd::Aborted));
            rikka_ui::clear_drag_payload();
        }
        let request = windows
            .hosting
            .get_mut(&id)
            .and_then(|hosted| hosted.ui.take_drag_window());
        if let Some(request) = request {
            self.carry_window(windows, id, request);
        }
    }

    pub(crate) fn handle_event(&mut self, windows: &mut Windows<'_>, event: DragEvent) {
        match event {
            DragEvent::Entered {
                surface,
                x,
                y,
                delivery,
            } => {
                self.placed();
                self.entered(windows, &surface, logical_point(x, y), delivery);
            }
            DragEvent::Moved { x, y } => {
                let Some(hovering) = self.hovering.as_mut() else {
                    return;
                };
                hovering.position = logical_point(x, y);
                self.moved(windows);
            }
            DragEvent::Left => {
                self.placed();
                self.left(windows);
            }
            DragEvent::Dropped { action } => {
                self.dropped(windows, action.unwrap_or(DragAction::Copy));
            }
            DragEvent::Ended(end) => self.ended(windows, end),
        }
    }

    pub(crate) fn settle_within(&mut self, windows: &mut Windows<'_>) {
        let within = self.hovering.is_some();
        let Some(active) = self
            .active
            .as_mut()
            .filter(|active| !active.announced && active.moves_windows)
        else {
            return;
        };
        if active.within.is_none_or(|was| was == within) {
            return;
        }
        active.within = Some(within);
        let origin = active.origin;
        if within && let Some(Following::Opened(detached)) = active.following {
            active.following = Some(Following::Returned(detached));
            windows.dispatch_event(detached, Event::DragReclaimed);
        }
        windows.dispatch_event(origin, Event::DragWithin(within));
        self.begin_from(windows, origin);
    }

    fn placed(&mut self) {
        if let Some(active) = self.active.as_mut() {
            active.within.get_or_insert(true);
        }
    }

    pub(crate) fn opened(&mut self, windows: &mut Windows<'_>, id: WindowId) {
        let Some(active) = self.active.as_mut() else {
            return;
        };
        let Some(grip) = active.opening.take() else {
            return;
        };
        if attach(windows, id, grip) {
            active.following = Some(Following::Opened(id));
            windows.withhold(id, true);
        }
    }

    pub(crate) fn cancel(&mut self, session: &Session) {
        if self.active.is_none() {
            return;
        }
        if let Some(drags) = session.drags()
            && let Err(error) = drags.cancel()
        {
            tracing::debug!("failed to cancel drag: {error}");
        }
    }

    pub(crate) fn window_gone(&mut self, session: &Session, id: WindowId) {
        if let Some(active) = self.active.as_mut() {
            if active
                .following
                .and_then(Following::detached)
                .is_some_and(|detached| detached == id)
            {
                active.following = None;
            }
            if active.origin == id
                && let Some(drags) = session.drags()
                && let Err(error) = drags.cancel()
            {
                tracing::debug!("drag source window closed: {error}");
            }
        }
        if self
            .hovering
            .as_ref()
            .is_some_and(|hovering| hovering.window == id)
        {
            self.hovering = None;
            answer(session, None);
        }
    }

    pub(crate) fn paint_ghost(&mut self, hosting: &mut Hosting) -> Option<Instant> {
        let ghost = self.active.as_mut()?.ghost.as_mut()?;
        ghost.settle_pictures();
        if ghost.wants_draw()
            && let Some(mut renderer) = hosting.renderer()
        {
            ghost.draw(&mut renderer);
        }
        ghost
            .loads_pictures()
            .then(|| Instant::now() + PICTURE_POLL)
    }

    pub(crate) fn release(&mut self) {
        self.active = None;
        self.hovering = None;
    }

    fn take_up(&mut self, windows: &mut Windows<'_>, id: WindowId, start: DragStart) -> bool {
        let Some(drags) = windows.session.drags() else {
            return false;
        };
        let Some((origin, scale)) = windows.hosting.get(&id).map(|hosted| {
            (
                hosted.state.toplevel.surface().clone(),
                hosted.state.scale().factor(),
            )
        }) else {
            return false;
        };
        let DragStart {
            offer,
            actions,
            icon,
            moves_windows,
        } = start;
        let scale = buffer_scale(scale);
        let request = data_device::Start {
            entries: offer.entries().to_vec(),
            actions,
            origin,
            icon: Some(asked_for(&icon, scale)),
            moves_windows,
        };
        let started = match drags.start(request) {
            Ok(started) => started,
            Err(error) => {
                tracing::warn!("failed to start drag: {error}");
                return false;
            }
        };
        let ghost = ghost_of(windows, started.icon, icon, scale);
        self.active = Some(ActiveDrag {
            origin: id,
            offer,
            announced: false,
            ghost,
            moves_windows: started.moves_windows,
            within: None,
            opening: None,
            following: None,
        });
        true
    }

    fn carry_window(&mut self, windows: &mut Windows<'_>, id: WindowId, request: DragWindow) {
        let Some(active) = self
            .active
            .as_mut()
            .filter(|active| active.moves_windows && active.following.is_none())
        else {
            return;
        };
        match request {
            DragWindow::Opening { grip } => active.opening = Some(grip),
            DragWindow::This { grip } => {
                if active.origin == id && attach(windows, id, grip) {
                    active.following = Some(Following::Origin);
                    if let Some(ghost) = active.ghost.as_mut() {
                        ghost.hide();
                    }
                }
            }
        }
    }

    fn entered(
        &mut self,
        windows: &mut Windows<'_>,
        surface: &WlSurface,
        position: Point,
        delivery: Delivery,
    ) {
        self.left(windows);
        let Some(window) = windows.drawing_on(surface) else {
            answer(windows.session, None);
            return;
        };
        let offer = DragOffer::new(delivery.mimes().iter().cloned()).actions(delivery.actions());
        let offer = match self.active.as_ref() {
            Some(active) => offer.ours(true).delivered_by(Local {
                offer: active.offer.clone(),
                _wire: delivery,
            }),
            None => offer.delivered_by(Wire(delivery)),
        };
        self.hovering = Some(Hovering {
            window,
            offer,
            position,
            accepted: None,
        });
        self.moved(windows);
    }

    fn moved(&mut self, windows: &mut Windows<'_>) {
        let Some(over) = self.hovering.as_mut() else {
            return;
        };
        let window = over.window;
        let Some(hosted) = windows.hosting.get_mut(&window) else {
            return;
        };
        hosted.ui.handle_event(Event::DragMoved {
            position: over.position,
            offer: over.offer.clone(),
        });
        let answered = hosted.ui.drag_accepts();
        if over.accepted.as_ref() != answered {
            over.accepted = answered.cloned();
            answer(windows.session, over.accepted.as_ref());
        }
        windows.service(window);
    }

    fn left(&mut self, windows: &mut Windows<'_>) {
        let Some(hovering) = self.hovering.take() else {
            return;
        };
        windows.dispatch_event(hovering.window, Event::DragLeft);
    }

    fn dropped(&mut self, windows: &mut Windows<'_>, action: DragAction) {
        let Some(over) = self.hovering.take() else {
            return;
        };
        if over.accepted.is_none() {
            windows.dispatch_event(over.window, Event::DragLeft);
            return;
        }
        let origin = self.active.as_ref().map(|active| active.origin);
        if let Some(origin) = origin {
            windows.dispatch_event(origin, Event::DragTaken);
        }
        windows.dispatch_event(
            over.window,
            Event::DragDropped {
                position: over.position,
                offer: over.offer,
                action,
            },
        );
        if origin.is_some() {
            self.settle(windows, DragEnd::Landed(action));
        }
    }

    fn ended(&mut self, windows: &mut Windows<'_>, end: End) {
        let end = match end {
            End::Landed(action) => DragEnd::Landed(action),
            End::Nowhere => DragEnd::Nowhere,
            End::Aborted => DragEnd::Aborted,
        };
        let following = self.active.as_ref().and_then(|active| active.following);
        if let Some(detached) = following.and_then(Following::detached) {
            if end == DragEnd::Aborted && following == Some(Following::Opened(detached)) {
                windows.dispatch_event(detached, Event::DragReclaimed);
            }
            if !windows.closes(detached) {
                windows.withhold(detached, false);
            }
        }
        self.settle(windows, end);
        self.active = None;
        self.left(windows);
        if let Some(drags) = windows.session.drags()
            && let Err(error) = drags.release_icon()
        {
            tracing::debug!("drag icon outlived drag: {error}");
        }
    }

    fn settle(&mut self, windows: &mut Windows<'_>, end: DragEnd) {
        let Some(active) = self.active.as_mut().filter(|active| !active.announced) else {
            return;
        };
        active.announced = true;
        let origin = active.origin;
        windows.dispatch_event(origin, Event::DragEnded(end));
        rikka_ui::clear_drag_payload();
    }
}

impl std::fmt::Debug for Dragging {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Dragging")
            .field("active", &self.active.is_some())
            .field("hovering", &self.hovering.is_some())
            .finish_non_exhaustive()
    }
}

fn attach(windows: &Windows<'_>, id: WindowId, grip: Point) -> bool {
    let (Some(drags), Some(hosted)) = (windows.session.drags(), windows.hosting.get(&id)) else {
        return false;
    };
    let toplevel = hosted.state.toplevel.xdg_toplevel();
    let attached = drags.attach(toplevel, grip.x.round() as i32, grip.y.round() as i32);
    match attached {
        Ok(()) => true,
        Err(error) => {
            tracing::warn!("failed to attach window to drag: {error}");
            false
        }
    }
}

fn ghost_of(
    windows: &mut Windows<'_>,
    surface: Option<WlSurface>,
    icon: DragIcon,
    scale: f32,
) -> Option<Ghost> {
    match (surface, windows.hosting.renderer()) {
        (Some(surface), Some(mut renderer)) => {
            match Ghost::new(&mut renderer, &windows.client.host, &surface, icon, scale) {
                Ok(ghost) => Some(ghost),
                Err(error) => {
                    tracing::warn!("failed to create drag icon: {error}");
                    None
                }
            }
        }
        _ => None,
    }
}

fn answer(session: &Session, accept: Option<&Accept>) {
    if let Some(drags) = session.drags()
        && let Err(error) = drags.accept(accept.map(wire::answer))
    {
        tracing::debug!("drag request got no reply: {error}");
    }
}

fn asked_for(icon: &DragIcon, scale: f32) -> Icon {
    Icon {
        scale: scale as i32,
        hotspot_x: icon.hotspot.x.round() as i32,
        hotspot_y: icon.hotspot.y.round() as i32,
    }
}

fn buffer_scale(scale: f64) -> f32 {
    (scale.ceil() as f32).max(1.0)
}

fn logical_point(x: f64, y: f64) -> Point {
    Point::new(x as f32, y as f32)
}

fn pixels(logical: f32, scale: f32) -> u32 {
    (logical * scale).round().max(1.0) as u32
}
