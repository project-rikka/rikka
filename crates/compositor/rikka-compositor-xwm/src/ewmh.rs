use x11rb_protocol::protocol::xproto::Atom;

use crate::atoms::Atoms;
use crate::window::{WindowStates, WindowType};

pub(crate) fn state_atoms(atoms: &Atoms) -> [(Atom, WindowStates); 13] {
    [
        (atoms._NET_WM_STATE_MODAL, WindowStates::MODAL),
        (atoms._NET_WM_STATE_STICKY, WindowStates::STICKY),
        (
            atoms._NET_WM_STATE_MAXIMIZED_VERT,
            WindowStates::MAXIMIZED_VERT,
        ),
        (
            atoms._NET_WM_STATE_MAXIMIZED_HORZ,
            WindowStates::MAXIMIZED_HORZ,
        ),
        (atoms._NET_WM_STATE_SHADED, WindowStates::SHADED),
        (atoms._NET_WM_STATE_SKIP_TASKBAR, WindowStates::SKIP_TASKBAR),
        (atoms._NET_WM_STATE_SKIP_PAGER, WindowStates::SKIP_PAGER),
        (atoms._NET_WM_STATE_HIDDEN, WindowStates::HIDDEN),
        (atoms._NET_WM_STATE_FULLSCREEN, WindowStates::FULLSCREEN),
        (atoms._NET_WM_STATE_ABOVE, WindowStates::ABOVE),
        (atoms._NET_WM_STATE_BELOW, WindowStates::BELOW),
        (
            atoms._NET_WM_STATE_DEMANDS_ATTENTION,
            WindowStates::DEMANDS_ATTENTION,
        ),
        (atoms._NET_WM_STATE_FOCUSED, WindowStates::FOCUSED),
    ]
}

pub(crate) fn type_atoms(atoms: &Atoms) -> [(Atom, WindowType); 14] {
    [
        (atoms._NET_WM_WINDOW_TYPE_NORMAL, WindowType::Normal),
        (atoms._NET_WM_WINDOW_TYPE_DIALOG, WindowType::Dialog),
        (atoms._NET_WM_WINDOW_TYPE_UTILITY, WindowType::Utility),
        (atoms._NET_WM_WINDOW_TYPE_TOOLBAR, WindowType::Toolbar),
        (atoms._NET_WM_WINDOW_TYPE_SPLASH, WindowType::Splash),
        (atoms._NET_WM_WINDOW_TYPE_MENU, WindowType::Menu),
        (
            atoms._NET_WM_WINDOW_TYPE_DROPDOWN_MENU,
            WindowType::DropdownMenu,
        ),
        (atoms._NET_WM_WINDOW_TYPE_POPUP_MENU, WindowType::PopupMenu),
        (atoms._NET_WM_WINDOW_TYPE_TOOLTIP, WindowType::Tooltip),
        (
            atoms._NET_WM_WINDOW_TYPE_NOTIFICATION,
            WindowType::Notification,
        ),
        (atoms._NET_WM_WINDOW_TYPE_COMBO, WindowType::Combo),
        (atoms._NET_WM_WINDOW_TYPE_DND, WindowType::Dnd),
        (atoms._NET_WM_WINDOW_TYPE_DESKTOP, WindowType::Desktop),
        (atoms._NET_WM_WINDOW_TYPE_DOCK, WindowType::Dock),
    ]
}

pub(crate) fn states_from(atoms: &Atoms, values: impl IntoIterator<Item = Atom>) -> WindowStates {
    let table = state_atoms(atoms);
    values
        .into_iter()
        .filter_map(|value| table.iter().find(|(atom, _)| *atom == value))
        .fold(WindowStates::empty(), |states, (_, state)| states | *state)
}

pub(crate) fn state_list(atoms: &Atoms, states: WindowStates) -> Vec<Atom> {
    state_atoms(atoms)
        .iter()
        .filter(|(_, state)| states.contains(*state))
        .map(|(atom, _)| *atom)
        .collect()
}

pub(crate) fn state_of(atoms: &Atoms, atom: Atom) -> WindowStates {
    states_from(atoms, [atom])
}

pub(crate) fn types_from(atoms: &Atoms, values: impl IntoIterator<Item = Atom>) -> Vec<WindowType> {
    let table = type_atoms(atoms);
    values
        .into_iter()
        .filter_map(|value| {
            table
                .iter()
                .find(|(atom, _)| *atom == value)
                .map(|(_, kind)| *kind)
        })
        .collect()
}

pub(crate) fn supported(atoms: &Atoms) -> Vec<Atom> {
    let mut supported = vec![
        atoms._NET_SUPPORTED,
        atoms._NET_SUPPORTING_WM_CHECK,
        atoms._NET_ACTIVE_WINDOW,
        atoms._NET_CLIENT_LIST,
        atoms._NET_CLIENT_LIST_STACKING,
        atoms._NET_WM_NAME,
        atoms._NET_WM_PID,
        atoms._NET_WM_MOVERESIZE,
        atoms._NET_FRAME_EXTENTS,
        atoms._NET_REQUEST_FRAME_EXTENTS,
        atoms._NET_WM_WINDOW_OPACITY,
        atoms._NET_WM_STATE,
        atoms._NET_WM_WINDOW_TYPE,
    ];
    supported.extend(
        state_atoms(atoms)
            .iter()
            .map(|(atom, _)| *atom)
            .filter(|atom| {
                ![
                    atoms._NET_WM_STATE_STICKY,
                    atoms._NET_WM_STATE_SHADED,
                    atoms._NET_WM_STATE_BELOW,
                ]
                .contains(atom)
            }),
    );
    supported.extend(type_atoms(atoms).iter().map(|(atom, _)| *atom));
    supported
}
