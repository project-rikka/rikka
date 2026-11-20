use rikka_types::DragAction;
use x11rb_protocol::protocol::xproto::Atom;

use crate::atoms::Atoms;

pub(crate) fn atom(action: DragAction, atoms: &Atoms) -> Atom {
    match action {
        DragAction::Copy => atoms.XdndActionCopy,
        DragAction::Move => atoms.XdndActionMove,
        DragAction::Ask => atoms.XdndActionAsk,
    }
}

pub(crate) fn from_atom(atom: Atom, atoms: &Atoms) -> Option<DragAction> {
    match atom {
        atom if atom == atoms.XdndActionCopy => Some(DragAction::Copy),
        atom if atom == atoms.XdndActionMove => Some(DragAction::Move),
        atom if atom == atoms.XdndActionAsk => Some(DragAction::Ask),
        atom if atom == atoms.XdndActionLink || atom == atoms.XdndActionPrivate => {
            Some(DragAction::Copy)
        }
        _ => None,
    }
}
