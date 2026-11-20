use rikka_compositor_shell::Shell;

use crate::cursor::Cursor;
use crate::session::Session;
use crate::settings::SessionSettings;
use crate::state::Core;
use crate::x11::X11;

pub(crate) struct SessionRef<'a> {
    pub(crate) core: &'a Core,
    pub(crate) shell: &'a Shell,
    pub(crate) cursor: &'a Cursor,
    pub(crate) settings: &'a Option<SessionSettings>,
    pub(crate) session: &'a Session,
    pub(crate) x11: &'a X11,
}

macro_rules! session_ref {
    ($host:expr) => {
        $crate::context::SessionRef {
            core: &$host.core,
            shell: &$host.shell,
            cursor: &$host.cursor,
            settings: &$host.settings,
            session: &$host.session,
            x11: &$host.x11,
        }
    };
}

pub(crate) use session_ref;
