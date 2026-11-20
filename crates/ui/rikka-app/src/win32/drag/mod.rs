mod data;
mod formats;
mod ghost;
mod source;
pub(crate) mod target;

use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use rikka_clipboard_core::Offer;
use rikka_types::{DragAction, DragActions, Point};
use rikka_ui::{Accept, Delivery, DragEnd, DragOffer, DragStart, DragWindow, Event};
use windows::Win32::Foundation::{DRAGDROP_S_DROP, HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::{ClientToScreen, ScreenToClient};
use windows::Win32::System::Com::IDataObject;
use windows::Win32::System::Ole::{
    DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_MOVE, DROPEFFECT_NONE, IDropSource,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetWindowRect, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SetWindowPos,
};
use windows::core::HRESULT;

use crate::tasks::WindowId;
use crate::win32::drag::data::DataObject;
use crate::win32::drag::ghost::Ghost;
use crate::win32::drag::source::DropSource;
use crate::win32::hosting::{Hosting, Windows};
use crate::win32::input::{logical, scale_of};

const PICTURE_POLL: Duration = Duration::from_millis(16);
const DROPPED_DATA_LIMIT: usize = 256 * 1024 * 1024;

#[derive(Default)]
pub(crate) struct Dragging {
    active: Option<ActiveDrag>,
    hovering: Option<Hovering>,
    looping: Option<DragLoop>,
}

pub(crate) struct DragLoop {
    pub(crate) data: IDataObject,
    pub(crate) source: IDropSource,
    pub(crate) allowed: DROPEFFECT,
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
    escaped: Rc<Cell<bool>>,
}

#[derive(Clone, Copy, PartialEq)]
enum Following {
    Opened(WindowId, Point),
    Returned(WindowId, Point),
    Origin(Point),
}

impl Following {
    fn detached(self) -> Option<(WindowId, Point)> {
        match self {
            Self::Opened(window, grip) | Self::Returned(window, grip) => Some((window, grip)),
            Self::Origin(_) => None,
        }
    }
}

struct Hovering {
    window: WindowId,
    offer: DragOffer,
    position: Point,
    accepted: Option<Accept>,
    data: IDataObject,
    ours: bool,
}

impl Dragging {
    pub(crate) fn begin_from(&mut self, windows: &mut Windows<'_>, id: WindowId) {
        if let Some(start) = windows
            .hosting
            .get_mut(&id)
            .and_then(|hosted| hosted.ui.take_drag_start())
            && (self.active.is_some() || !self.take_up(windows.hosting, id, start))
        {
            windows.dispatch(id, Event::DragEnded(DragEnd::Aborted));
            rikka_ui::clear_drag_payload();
        }
        let request = windows
            .hosting
            .get_mut(&id)
            .and_then(|hosted| hosted.ui.take_drag_window());
        if let Some(request) = request {
            self.carry_window(windows.hosting, id, request);
        }
    }

    pub(crate) fn take_loop(&mut self) -> Option<DragLoop> {
        self.looping.take()
    }

    pub(crate) fn finished(
        &mut self,
        windows: &mut Windows<'_>,
        result: HRESULT,
        effect: DROPEFFECT,
    ) {
        let escaped = self
            .active
            .as_ref()
            .is_some_and(|active| active.escaped.get());
        let end = match (
            result == DRAGDROP_S_DROP,
            effect == DROPEFFECT_NONE,
            escaped,
        ) {
            (true, false, _) => DragEnd::Landed(action_of(effect)),
            (_, _, true) => DragEnd::Aborted,
            _ => DragEnd::Nowhere,
        };
        let following = self.active.as_ref().and_then(|active| active.following);
        if let Some((detached, grip)) = following.and_then(Following::detached) {
            if end == DragEnd::Aborted && following == Some(Following::Opened(detached, grip)) {
                windows.dispatch(detached, Event::DragReclaimed);
            }
            if let Some(hosted) = windows.hosting.get(&detached) {
                place_under_cursor(hosted.state.hwnd(), grip);
            }
            windows.withhold(detached, false);
        }
        self.settle(windows, end);
        self.active = None;
        self.left_any(windows);
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
        if within && let Some(Following::Opened(detached, grip)) = active.following {
            active.following = Some(Following::Returned(detached, grip));
            windows.dispatch(detached, Event::DragReclaimed);
        }
        windows.dispatch(origin, Event::DragWithin(within));
        self.begin_from(windows, origin);
    }

    pub(crate) fn opened(&mut self, windows: &mut Windows<'_>, id: WindowId) {
        let Some(active) = self.active.as_mut() else {
            return;
        };
        let Some(grip) = active.opening.take() else {
            return;
        };
        active.following = Some(Following::Opened(id, grip));
        windows.withhold(id, true);
    }

    pub(crate) fn window_gone(&mut self, id: WindowId) {
        if let Some(active) = self.active.as_mut()
            && active
                .following
                .and_then(Following::detached)
                .is_some_and(|(detached, _)| detached == id)
        {
            active.following = None;
        }
        if self
            .hovering
            .as_ref()
            .is_some_and(|hovering| hovering.window == id)
        {
            self.hovering = None;
        }
    }

    pub(crate) fn step(&mut self, hosting: &Hosting) {
        let Some(active) = self.active.as_ref() else {
            return;
        };
        let mut cursor = POINT::default();
        if let Err(error) = unsafe { GetCursorPos(&mut cursor) } {
            tracing::debug!("failed to read the pointer position: {error}");
            return;
        }
        if let Some(ghost) = active.ghost.as_ref() {
            ghost.follow(cursor);
        }
        if let Some(Following::Origin(grip)) = active.following
            && let Some(hosted) = hosting.get(&active.origin)
        {
            place_under_cursor(hosted.state.hwnd(), grip);
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
        self.looping = None;
        self.active = None;
        self.hovering = None;
    }

    pub(crate) fn entered(
        &mut self,
        windows: &mut Windows<'_>,
        window: WindowId,
        data: IDataObject,
        screen: (i32, i32),
        allowed: DROPEFFECT,
    ) -> DROPEFFECT {
        self.left_any(windows);
        self.placed();
        let Some(hwnd) = windows
            .hosting
            .get(&window)
            .map(|hosted| hosted.state.hwnd())
        else {
            return DROPEFFECT_NONE;
        };
        let ours = self.active.is_some() && formats::is_ours(&data);
        let offer = DragOffer::new(formats::mimes(&data)).actions(actions_of(allowed));
        let offer = match self.active.as_ref().filter(|_| ours) {
            Some(active) => offer.ours(true).delivered_by(Local {
                offer: active.offer.clone(),
            }),
            None => offer,
        };
        self.hovering = Some(Hovering {
            window,
            offer,
            position: client_position(hwnd, screen),
            accepted: None,
            data,
            ours,
        });
        self.moved(windows, window, screen, allowed)
    }

    pub(crate) fn moved(
        &mut self,
        windows: &mut Windows<'_>,
        window: WindowId,
        screen: (i32, i32),
        allowed: DROPEFFECT,
    ) -> DROPEFFECT {
        let Some(over) = self.hovering.as_mut().filter(|over| over.window == window) else {
            return DROPEFFECT_NONE;
        };
        let Some(hosted) = windows.hosting.get_mut(&window) else {
            return DROPEFFECT_NONE;
        };
        over.position = client_position(hosted.state.hwnd(), screen);
        hosted.ui.handle_event(Event::DragMoved {
            position: over.position,
            offer: over.offer.clone(),
        });
        over.accepted = hosted.ui.drag_accepts().cloned();
        let effect = effect_for(over.accepted.as_ref(), allowed);
        windows.touch(window);
        effect
    }

    pub(crate) fn left(&mut self, windows: &mut Windows<'_>, window: WindowId) {
        if self
            .hovering
            .as_ref()
            .is_some_and(|hovering| hovering.window == window)
        {
            self.placed();
            self.left_any(windows);
        }
    }

    pub(crate) fn dropped(
        &mut self,
        windows: &mut Windows<'_>,
        window: WindowId,
        screen: (i32, i32),
        allowed: DROPEFFECT,
    ) -> DROPEFFECT {
        let effect = self.moved(windows, window, screen, allowed);
        let Some(over) = self.hovering.take() else {
            return DROPEFFECT_NONE;
        };
        if over.accepted.is_none() || effect == DROPEFFECT_NONE {
            windows.dispatch(over.window, Event::DragLeft);
            return DROPEFFECT_NONE;
        }
        let action = action_of(effect);
        let offer = match over.ours {
            true => over.offer,
            false => {
                let read: HashMap<String, Vec<u8>> = over
                    .offer
                    .mimes()
                    .iter()
                    .filter_map(|mime| {
                        let bytes = formats::read(&over.data, mime, DROPPED_DATA_LIMIT)?;
                        Some((mime.clone(), bytes))
                    })
                    .collect();
                DragOffer::new(over.offer.mimes().iter().cloned())
                    .actions(over.offer.source_actions())
                    .delivered_by(Memory(read))
            }
        };
        let origin = self.active.as_ref().map(|active| active.origin);
        if let Some(origin) = origin {
            windows.dispatch(origin, Event::DragTaken);
        }
        windows.dispatch(
            over.window,
            Event::DragDropped {
                position: over.position,
                offer,
                action,
            },
        );
        if origin.is_some() {
            self.settle(windows, DragEnd::Landed(action));
        }
        effect
    }

    fn take_up(&mut self, hosting: &mut Hosting, id: WindowId, start: DragStart) -> bool {
        let Some(scale) = hosting
            .get(&id)
            .map(|hosted| scale_of(hosted.state.hwnd()) as f32)
        else {
            return false;
        };
        let DragStart {
            offer,
            actions,
            icon,
            moves_windows,
        } = start;
        let ghost = match hosting.renderer() {
            Some(mut renderer) => match Ghost::new(&mut renderer, icon, scale) {
                Ok(ghost) => Some(ghost),
                Err(error) => {
                    tracing::warn!("failed to create drag icon: {error}");
                    None
                }
            },
            None => None,
        };
        let escaped = Rc::new(Cell::new(false));
        self.looping = Some(DragLoop {
            data: DataObject::new(formats::entries(&offer)).into(),
            source: DropSource::new(Rc::clone(&escaped)).into(),
            allowed: effects_of(actions),
        });
        self.active = Some(ActiveDrag {
            origin: id,
            offer,
            announced: false,
            ghost,
            moves_windows,
            within: None,
            opening: None,
            following: None,
            escaped,
        });
        true
    }

    fn carry_window(&mut self, hosting: &Hosting, id: WindowId, request: DragWindow) {
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
                if active.origin == id && hosting.contains(&id) {
                    active.following = Some(Following::Origin(grip));
                    if let Some(ghost) = active.ghost.as_mut() {
                        ghost.hide();
                    }
                }
            }
        }
    }

    fn placed(&mut self) {
        if let Some(active) = self.active.as_mut() {
            active.within.get_or_insert(true);
        }
    }

    fn left_any(&mut self, windows: &mut Windows<'_>) {
        if let Some(hovering) = self.hovering.take() {
            windows.dispatch(hovering.window, Event::DragLeft);
        }
    }

    fn settle(&mut self, windows: &mut Windows<'_>, end: DragEnd) {
        let Some(active) = self.active.as_mut().filter(|active| !active.announced) else {
            return;
        };
        active.announced = true;
        let origin = active.origin;
        windows.dispatch(origin, Event::DragEnded(end));
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

struct Local {
    offer: Offer,
}

impl Delivery for Local {
    fn read(&self, mime: &str) -> std::io::Result<Vec<u8>> {
        self.offer
            .data(mime)
            .map(|data| data.to_vec())
            .ok_or_else(|| std::io::Error::other(format!("drag does not offer `{mime}`")))
    }
}

struct Memory(HashMap<String, Vec<u8>>);

impl Delivery for Memory {
    fn read(&self, mime: &str) -> std::io::Result<Vec<u8>> {
        self.0
            .get(mime)
            .cloned()
            .ok_or_else(|| std::io::Error::other(format!("drop did not carry `{mime}`")))
    }
}

fn client_position(window: HWND, screen: (i32, i32)) -> Point {
    let mut at = POINT {
        x: screen.0,
        y: screen.1,
    };
    if !unsafe { ScreenToClient(window, &mut at) }.as_bool() {
        return Point::ZERO;
    }
    logical(window, at.x, at.y)
}

fn place_under_cursor(window: HWND, grip: Point) {
    let mut cursor = POINT::default();
    let mut frame = RECT::default();
    let mut client = POINT::default();
    let measured = unsafe {
        GetCursorPos(&mut cursor)
            .and_then(|()| GetWindowRect(window, &mut frame))
            .map(|()| ClientToScreen(window, &mut client).as_bool())
    };
    if !matches!(measured, Ok(true)) {
        return;
    }
    let scale = scale_of(window);
    let grip_x = (f64::from(grip.x) * scale).round() as i32;
    let grip_y = (f64::from(grip.y) * scale).round() as i32;
    let left = cursor.x - grip_x - (client.x - frame.left);
    let top = cursor.y - grip_y - (client.y - frame.top);
    if let Err(error) = unsafe {
        SetWindowPos(
            window,
            None,
            left,
            top,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        )
    } {
        tracing::debug!("failed to move the carried window: {error}");
    }
}

fn actions_of(effect: DROPEFFECT) -> DragActions {
    let copy = match effect.0 & DROPEFFECT_COPY.0 != 0 {
        true => DragActions::COPY,
        false => DragActions::NONE,
    };
    let moves = match effect.0 & DROPEFFECT_MOVE.0 != 0 {
        true => DragActions::MOVE,
        false => DragActions::NONE,
    };
    copy | moves
}

fn effects_of(actions: DragActions) -> DROPEFFECT {
    let mut effect = DROPEFFECT_NONE.0;
    if actions.contains(DragAction::Copy) || actions.contains(DragAction::Ask) {
        effect |= DROPEFFECT_COPY.0;
    }
    if actions.contains(DragAction::Move) || actions.contains(DragAction::Ask) {
        effect |= DROPEFFECT_MOVE.0;
    }
    DROPEFFECT(effect)
}

fn effect_of(action: DragAction) -> DROPEFFECT {
    match action {
        DragAction::Move => DROPEFFECT_MOVE,
        DragAction::Copy | DragAction::Ask => DROPEFFECT_COPY,
    }
}

fn effect_for(accepted: Option<&Accept>, allowed: DROPEFFECT) -> DROPEFFECT {
    let Some(accepted) = accepted else {
        return DROPEFFECT_NONE;
    };
    let wanted = effects_of(accepted.actions).0 & allowed.0;
    let preferred = effect_of(accepted.preferred);
    if wanted & preferred.0 != 0 {
        return preferred;
    }
    [DROPEFFECT_COPY, DROPEFFECT_MOVE]
        .into_iter()
        .find(|effect| wanted & effect.0 != 0)
        .unwrap_or(DROPEFFECT_NONE)
}

fn action_of(effect: DROPEFFECT) -> DragAction {
    match effect.0 & DROPEFFECT_MOVE.0 != 0 {
        true => DragAction::Move,
        false => DragAction::Copy,
    }
}
