use rikka_seam::{Call, Manifest, Method, MethodInfo, Service};
use rikka_session::desktop::{CAPABILITY, RESTART, START, STATUS, STOP, VERSION};

use crate::context::session_ctx;
use crate::state::Compositor;

#[derive(Debug)]
pub(crate) struct DesktopService;

impl Service<Compositor> for DesktopService {
    fn manifest(&self) -> Manifest {
        Manifest::new(CAPABILITY, VERSION)
            .summary("Desktop shell process supervised by the session")
            .method(MethodInfo::new(STATUS).summary("Shell program path and process id"))
            .method(MethodInfo::new(START).summary("Start the shell if it is not running"))
            .method(MethodInfo::new(STOP).summary("Stop the shell, keeping windows open"))
            .method(MethodInfo::new(RESTART).summary("Restart the shell, picking up a new build"))
    }

    fn call(&mut self, compositor: &mut Compositor, method: Method, call: Call) {
        match &*method {
            STATUS => call.encode(&compositor.session.desktop.status()),
            START => {
                call.done();
                compositor.session.desktop.restarts.forget();
                session_ctx!(compositor).start_desktop();
            }
            STOP => {
                call.done();
                session_ctx!(compositor).stop_desktop();
            }
            RESTART => {
                call.done();
                compositor.session.desktop.restarts.forget();
                session_ctx!(compositor).restart_desktop();
            }
            _ => call.unknown_method(),
        }
    }
}
