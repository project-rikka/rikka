use rikka_compositor_shell::Shell;

use crate::cursor::Cursor;
use crate::session::Session;
use crate::settings::SessionSettings;
use crate::state::Core;
use crate::x11::X11;

pub(crate) struct SessionCtx<'a> {
    pub(crate) core: &'a mut Core,
    pub(crate) shell: &'a mut Shell,
    pub(crate) cursor: &'a mut Cursor,
    pub(crate) settings: &'a mut Option<SessionSettings>,
    pub(crate) session: &'a mut Session,
    pub(crate) x11: &'a mut X11,
}

macro_rules! session_ctx {
    ($host:expr) => {
        $crate::context::SessionCtx {
            core: &mut $host.core,
            shell: &mut $host.shell,
            cursor: &mut $host.cursor,
            settings: &mut $host.settings,
            session: &mut $host.session,
            x11: &mut $host.x11,
        }
    };
}

pub(crate) use session_ctx;
