use std::sync::Arc;

use rikka_ime::{Composing, Panel};
use rikka_seam::{Call, EventInfo, Manifest, Method, MethodInfo, Service, ServiceContext, Version};
use shell_kit::Host;

use crate::methods::InputMethods;

pub(crate) const CAPABILITY: &str = "rikka.ime";

pub(crate) const REPORT: &str = "report";

pub(crate) const CHANGED: &str = "changed";

#[derive(Debug)]
pub(super) struct ImeService {
    panel: Panel,
    composing: Arc<Composing>,
    methods: InputMethods,
    seam: Option<ServiceContext>,
}

impl ImeService {
    #[must_use]
    pub(crate) fn new(panel: Panel, methods: InputMethods) -> ImeService {
        let composing = panel.composing();
        ImeService {
            panel,
            composing,
            methods,
            seam: None,
        }
    }

    fn settle(&mut self) {
        self.methods.refresh_status_area();
        let composing = self.panel.composing();
        if Arc::ptr_eq(&composing, &self.composing) {
            return;
        }
        self.composing = composing;
        if let Some(seam) = &self.seam
            && let Err(error) = seam.publisher().emit(CHANGED, &())
        {
            tracing::warn!("failed to emit candidate window change: {error}");
        }
    }
}

impl Service<Host> for ImeService {
    fn manifest(&self) -> Manifest {
        Manifest::new(CAPABILITY, Version::new(1, 0))
            .summary("Input method candidate window")
            .method(MethodInfo::new(REPORT).summary("Panel state changed"))
            .event(
                EventInfo::new(CHANGED)
                    .summary("Candidate window state changed")
                    .latest(),
            )
    }

    fn attach(&mut self, seam: ServiceContext) {
        self.seam = Some(seam);
    }

    fn call(&mut self, _: &mut Host, method: Method, call: Call) {
        match &*method {
            REPORT => {
                self.settle();
                call.done();
            }
            _ => call.unknown_method(),
        }
    }
}
