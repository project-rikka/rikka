use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_protocols::wl_surface::WlSurface;
use rikka_compositor_surface::{Committed, Role, RoleError, SurfaceId, assign_role};
use rikka_wayland_wire::{ClientId, serial_not_before};

use crate::access::Access;
use crate::event::SeatEvent;
use crate::host::SeatHost;
use crate::ids::{SeatId, ToolId};
use crate::pointer::shape::CursorShape;
use crate::surface;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorImage {
    Hidden,
    Shape(CursorShape),
    Surface {
        surface: SurfaceId,
        hotspot: Point<i32, Logical>,
    },
}

impl Default for CursorImage {
    fn default() -> Self {
        Self::Shape(CursorShape::Default)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CursorOwner {
    Pointer,
    Tool(ToolId),
}

#[derive(Debug)]
pub(crate) struct CursorRole {
    seat: SeatId,
    owner: CursorOwner,
    hotspot: Point<i32, Logical>,
}

impl<S: SeatHost> Role<S> for CursorRole {
    const NAME: &'static str = "cursor";

    fn committed(state: &mut S, commit: &Committed) {
        if commit.offset == Point::zero() {
            return;
        }
        let Some(role) = state.surfaces().role_mut::<CursorRole>(commit.surface) else {
            return;
        };
        role.hotspot -= commit.offset;
        let (seat, owner, hotspot) = (role.seat, role.owner, role.hotspot);
        show(
            state,
            seat,
            owner,
            CursorImage::Surface {
                surface: commit.surface,
                hotspot,
            },
            Some(commit.surface),
        );
    }
}

pub(crate) fn set_cursor<S: SeatHost>(
    state: &mut S,
    seat: SeatId,
    owner: CursorOwner,
    surface: Option<WlSurface>,
    hotspot: Point<i32, Logical>,
) -> Result<(), RoleError> {
    let Some(surface) = surface else {
        show(state, seat, owner, CursorImage::Hidden, None);
        return Ok(());
    };
    let Some(id) = surface::id(state, &surface) else {
        return Err(RoleError::Gone);
    };
    match state.surfaces().role_mut::<CursorRole>(id) {
        Some(role) => {
            role.seat = seat;
            role.owner = owner;
            role.hotspot = hotspot;
        }
        None => assign_role(
            state,
            id,
            CursorRole {
                seat,
                owner,
                hotspot,
            },
        )?,
    }
    show(
        state,
        seat,
        owner,
        CursorImage::Surface {
            surface: id,
            hotspot,
        },
        None,
    );
    Ok(())
}

pub(crate) fn show<A: Access>(
    state: &mut A,
    seat: SeatId,
    owner: CursorOwner,
    image: CursorImage,
    only_while: Option<SurfaceId>,
) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let current = match owner {
        CursorOwner::Pointer => &mut entry.pointer.cursor,
        CursorOwner::Tool(tool) => match entry.tablets.tools.get_mut(tool) {
            Some(tool) => &mut tool.cursor,
            None => return,
        },
    };
    let showing = |image: &CursorImage, surface: SurfaceId| matches!(image, CursorImage::Surface { surface: shown, .. } if *shown == surface);
    if only_while.is_some_and(|surface| !showing(current, surface)) {
        return;
    }
    if *current == image {
        return;
    }
    *current = image;
    let event = match owner {
        CursorOwner::Pointer => SeatEvent::Cursor { seat, image },
        CursorOwner::Tool(tool) => SeatEvent::ToolCursor { seat, tool, image },
    };
    state.report(event);
}

pub(crate) fn cursor_surface_gone<A: Access>(state: &mut A, seat: SeatId, surface: SurfaceId) {
    show(
        state,
        seat,
        CursorOwner::Pointer,
        CursorImage::default(),
        Some(surface),
    );
    let tools: Vec<ToolId> = state
        .seats()
        .get(seat)
        .map(|entry| entry.tablets.tools.keys().collect())
        .unwrap_or_default();
    for tool in tools {
        show(
            state,
            seat,
            CursorOwner::Tool(tool),
            CursorImage::default(),
            Some(surface),
        );
    }
}

pub(crate) fn may_set_cursor<A: Access>(
    state: &mut A,
    seat: SeatId,
    owner: CursorOwner,
    client: ClientId,
    serial: u32,
) -> bool {
    let Some(entry) = state.seats().get(seat) else {
        return false;
    };
    let (focused, entered) = match owner {
        CursorOwner::Pointer => (entry.pointer.focused.as_ref(), entry.pointer.enter_serial),
        CursorOwner::Tool(tool) => match entry.tablets.tools.get(tool) {
            Some(tool) => (tool.focused.as_ref(), tool.proximity_serial),
            None => return false,
        },
    };
    focused.is_some_and(|focused| focused.client() == client && focused.surface.alive())
        && serial_not_before(serial, entered)
}
