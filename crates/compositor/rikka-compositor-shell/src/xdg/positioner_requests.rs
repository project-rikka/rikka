use rikka_compositor_geometry::{Point, Rect, Size};
use rikka_compositor_protocols::xdg_positioner::{
    ConstraintAdjustment, Error, Request, XdgPositioner,
};

use super::positioner::Positioner;
use crate::host::ShellHost;
use crate::ids::PositionerId;
use crate::shell::Shell;

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Draft {
    positioner: Positioner,
    sized: bool,
    anchored: bool,
}

impl Draft {
    pub(crate) fn lookup(shell: &Shell, positioner: &XdgPositioner) -> Option<Positioner> {
        shell
            .positioners
            .get(positioner.key::<PositionerId>()?)
            .filter(|draft| draft.sized && draft.anchored)
            .map(|draft| draft.positioner)
    }
}

pub(crate) fn create<S: ShellHost>(state: &mut S, positioner: &XdgPositioner) {
    let draft = state.shell().positioners.insert(Draft::default());
    positioner.set_key(draft);
}

pub(crate) fn destroyed<S: ShellHost>(state: &mut S, positioner: XdgPositioner) {
    if let Some(key) = positioner.key::<PositionerId>() {
        state.shell().positioners.remove(key);
    }
}

pub(crate) fn request<S: ShellHost>(state: &mut S, positioner: XdgPositioner, request: Request) {
    let Some(draft) = positioner
        .key::<PositionerId>()
        .and_then(|key| state.shell().positioners.get_mut(key))
    else {
        return;
    };
    let mut edit = |change: &dyn Fn(&mut Draft)| change(draft);
    match request {
        Request::Destroy => {}
        Request::SetSize { width, height } => {
            if width <= 0 || height <= 0 {
                return positioner.post_error(Error::InvalidInput);
            }
            edit(&|draft| {
                draft.positioner.size = Size::new(width, height);
                draft.sized = true;
            });
        }
        Request::SetAnchorRect {
            x,
            y,
            width,
            height,
        } => {
            if width < 0 || height < 0 {
                return positioner.post_error(Error::InvalidInput);
            }
            edit(&|draft| {
                draft.positioner.anchor_rect =
                    Rect::new(Point::new(x, y), Size::new(width, height));
                draft.anchored = true;
            });
        }
        Request::SetAnchor { anchor } => match anchor {
            Ok(anchor) => edit(&|draft| draft.positioner.anchor = anchor),
            Err(_) => positioner.post_error(Error::InvalidInput),
        },
        Request::SetGravity { gravity } => match gravity {
            Ok(gravity) => edit(&|draft| draft.positioner.gravity = gravity),
            Err(_) => positioner.post_error(Error::InvalidInput),
        },
        Request::SetConstraintAdjustment {
            constraint_adjustment,
        } => {
            let known = ConstraintAdjustment::from_bits_truncate(constraint_adjustment.bits());
            edit(&|draft| draft.positioner.constraint_adjustment = known);
        }
        Request::SetOffset { x, y } => edit(&|draft| draft.positioner.offset = Point::new(x, y)),
        Request::SetReactive => edit(&|draft| draft.positioner.reactive = true),
        Request::SetParentSize {
            parent_width,
            parent_height,
        } => {
            if parent_width < 0 || parent_height < 0 {
                return positioner.post_error(Error::InvalidInput);
            }
            edit(&|draft| {
                draft.positioner.parent_size = Some(Size::new(parent_width, parent_height));
            });
        }
        Request::SetParentConfigure { serial } => {
            edit(&|draft| draft.positioner.parent_configure = Some(serial));
        }
    }
}
