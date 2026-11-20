use rikka_compositor_types::{Flip, Format, TearingHint};

use crate::assignment::{Assignment, Blocked, Reason};
use crate::display::Display;
use crate::layout::Layout;
use crate::surface::Surface;
use crate::tearing::Tearing;

pub fn assign<S: Copy>(
    display: &Display<'_>,
    covering: Option<&Surface<S>>,
    refused: impl Fn(Format) -> bool,
) -> Assignment<S> {
    let Some(surface) = covering else {
        return Assignment::Composited(None);
    };
    match primary(display, surface, refused) {
        Ok(layout) => Assignment::Primary(layout),
        Err(reason) => Assignment::Composited(Some(Blocked {
            surface: surface.id,
            reason,
        })),
    }
}

fn primary<S: Copy>(
    display: &Display<'_>,
    surface: &Surface<S>,
    refused: impl Fn(Format) -> bool,
) -> Result<Layout<S>, Reason> {
    if !display.formats().contains(surface.format) {
        return Err(Reason::Format);
    }
    if surface.transform != display.transform() {
        return Err(Reason::Transform);
    }
    let mode = display.mode();
    if (surface.buffer.w, surface.buffer.h) != (mode.w, mode.h) {
        return Err(Reason::Size);
    }
    if surface.source != surface.whole_buffer() {
        return Err(Reason::Source);
    }
    if surface.placement != display.whole() {
        return Err(Reason::Placement);
    }
    if !surface.opaque {
        return Err(Reason::Translucent);
    }
    if surface.covered {
        return Err(Reason::Covered);
    }
    if refused(surface.format) {
        return Err(Reason::Refused);
    }
    let flip = match (display.tearing(), surface.tearing) {
        (Tearing::Allowed, TearingHint::Async) => Flip::Async,
        (Tearing::Allowed, TearingHint::Vsync) | (Tearing::Forbidden, _) => Flip::Vsync,
    };
    Ok(Layout {
        surface: surface.id,
        format: surface.format,
        flip,
    })
}
