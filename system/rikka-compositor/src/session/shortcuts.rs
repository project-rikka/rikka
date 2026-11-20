use rikka_seam::{
    Call, EventInfo, Manifest, Method, MethodInfo, SeamError, Service, ServiceContext,
};
use rikka_session::shortcuts::{ACTIVATE, ACTIVATED, CAPABILITY, Shortcut, VERSION};

use crate::state::Compositor;

#[derive(Debug, Default)]
pub(crate) struct ShortcutsService {
    seam: Option<ServiceContext>,
}

impl ShortcutsService {
    pub(crate) fn new() -> Self {
        ShortcutsService::default()
    }

    fn activate(&mut self, call: Call) {
        let shortcut = match call.decode::<Shortcut>() {
            Ok(shortcut) => shortcut,
            Err(error) => return call.fail(error),
        };
        if !shortcut.is_published() {
            return call.fail(SeamError::InvalidParams(format!(
                "`{shortcut}` is handled by the compositor, not the shell"
            )));
        }
        let Some(seam) = &self.seam else {
            return call.fail(SeamError::Unavailable("no shortcut subscribers".to_owned()));
        };
        match seam.publisher().emit(ACTIVATED, &shortcut) {
            Ok(()) => call.done(),
            Err(error) => call.fail(error),
        }
    }
}

impl Service<Compositor> for ShortcutsService {
    fn manifest(&self) -> Manifest {
        Manifest::new(CAPABILITY, VERSION)
            .summary("Shortcuts the compositor forwards to the shell")
            .method(MethodInfo::new(ACTIVATE).summary("Trigger shortcut `name`"))
            .event(EventInfo::new(ACTIVATED).summary("Shortcut triggered"))
    }

    fn attach(&mut self, seam: ServiceContext) {
        self.seam = Some(seam);
    }

    fn call(&mut self, _: &mut Compositor, method: Method, call: Call) {
        match &*method {
            ACTIVATE => self.activate(call),
            _ => call.unknown_method(),
        }
    }
}
