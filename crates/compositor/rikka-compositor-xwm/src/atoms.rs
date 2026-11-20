use x11rb_protocol::protocol::xproto::Atom;

macro_rules! atoms {
    ($($name:ident),* $(,)?) => {
        #[allow(non_snake_case)]
        #[derive(Debug, Clone, Copy)]
        pub(crate) struct Atoms {
            $(pub(crate) $name: Atom,)*
        }

        impl Atoms {
            pub(crate) const NAMES: &[&str] = &[$(stringify!($name)),*];

            pub(crate) fn from_interned(mut interned: impl Iterator<Item = Atom>) -> Self {
                Self {
                    $($name: interned.next().unwrap_or(crate::wire::NONE),)*
                }
            }
        }
    };
}

atoms! {
    WM_PROTOCOLS,
    WM_DELETE_WINDOW,
    WM_TAKE_FOCUS,
    WM_STATE,
    WM_CHANGE_STATE,
    WM_S0,
    WL_SURFACE_SERIAL,
    UTF8_STRING,
    _NET_WM_CM_S0,
    _NET_SUPPORTED,
    _NET_SUPPORTING_WM_CHECK,
    _NET_ACTIVE_WINDOW,
    _NET_CLIENT_LIST,
    _NET_CLIENT_LIST_STACKING,
    _NET_WM_NAME,
    _NET_WM_PID,
    _NET_STARTUP_ID,
    _NET_WM_MOVERESIZE,
    _NET_WM_PING,
    _NET_WM_SYNC_REQUEST,
    _NET_FRAME_EXTENTS,
    _NET_REQUEST_FRAME_EXTENTS,
    _NET_WM_WINDOW_OPACITY,
    _NET_WM_STATE,
    _NET_WM_STATE_MODAL,
    _NET_WM_STATE_STICKY,
    _NET_WM_STATE_MAXIMIZED_VERT,
    _NET_WM_STATE_MAXIMIZED_HORZ,
    _NET_WM_STATE_SHADED,
    _NET_WM_STATE_SKIP_TASKBAR,
    _NET_WM_STATE_SKIP_PAGER,
    _NET_WM_STATE_HIDDEN,
    _NET_WM_STATE_FULLSCREEN,
    _NET_WM_STATE_ABOVE,
    _NET_WM_STATE_BELOW,
    _NET_WM_STATE_DEMANDS_ATTENTION,
    _NET_WM_STATE_FOCUSED,
    _NET_WM_WINDOW_TYPE,
    _NET_WM_WINDOW_TYPE_DESKTOP,
    _NET_WM_WINDOW_TYPE_DOCK,
    _NET_WM_WINDOW_TYPE_TOOLBAR,
    _NET_WM_WINDOW_TYPE_MENU,
    _NET_WM_WINDOW_TYPE_UTILITY,
    _NET_WM_WINDOW_TYPE_SPLASH,
    _NET_WM_WINDOW_TYPE_DIALOG,
    _NET_WM_WINDOW_TYPE_DROPDOWN_MENU,
    _NET_WM_WINDOW_TYPE_POPUP_MENU,
    _NET_WM_WINDOW_TYPE_TOOLTIP,
    _NET_WM_WINDOW_TYPE_NOTIFICATION,
    _NET_WM_WINDOW_TYPE_COMBO,
    _NET_WM_WINDOW_TYPE_DND,
    _NET_WM_WINDOW_TYPE_NORMAL,
    _MOTIF_WM_HINTS,
    _XSETTINGS_S0,
    _XSETTINGS_SETTINGS,
    MANAGER,
    CLIPBOARD,
    CLIPBOARD_MANAGER,
    PRIMARY,
    TARGETS,
    TIMESTAMP,
    INCR,
    DELETE,
    _WL_SELECTION,
    XdndAware,
    XdndProxy,
    XdndSelection,
    XdndTypeList,
    XdndEnter,
    XdndPosition,
    XdndStatus,
    XdndLeave,
    XdndDrop,
    XdndFinished,
    XdndActionCopy,
    XdndActionMove,
    XdndActionAsk,
    XdndActionLink,
    XdndActionPrivate,
}
