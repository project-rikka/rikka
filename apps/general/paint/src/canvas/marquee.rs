use paint_document::{Area, Combine};
use rikka_app::prelude::*;

pub(crate) fn combine_for(
    button: PointerButton,
    modifiers: Modifiers,
    mode: Combine,
) -> Option<Combine> {
    match (button, modifiers.control, modifiers.alt) {
        (PointerButton::Primary, false, false) => Some(mode),
        (PointerButton::Primary, true, false) => Some(Combine::Union),
        (PointerButton::Primary, _, true) => Some(Combine::Intersect),
        (PointerButton::Secondary, false, _) => Some(Combine::Exclude),
        (PointerButton::Secondary, true, _) => Some(Combine::Xor),
        _ => None,
    }
}

pub(crate) fn dragged_area(from: Point, to: Point, square: bool, extent: Size) -> Area {
    let to = match square {
        true => {
            let side = (to.x - from.x).abs().max((to.y - from.y).abs());
            Point::new(
                from.x + side.copysign(to.x - from.x),
                from.y + side.copysign(to.y - from.y),
            )
        }
        false => to,
    };
    let corner = |point: Point| {
        (
            point.x.round().clamp(0.0, extent.width) as u32,
            point.y.round().clamp(0.0, extent.height) as u32,
        )
    };
    Area::between(corner(from), corner(to))
}
