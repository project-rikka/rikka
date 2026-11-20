use rikka_seam::{Call, EventInfo, Manifest, Method, Publisher, Service};
use rikka_session::gestures::{CAPABILITY, SWIPED, Swipe, VERSION};

use crate::state::Compositor;

#[derive(Clone, Debug)]
pub(crate) struct Swipes(Publisher);

impl Swipes {
    pub(crate) fn new(publisher: Publisher) -> Self {
        Swipes(publisher)
    }

    pub(crate) fn announce(&self, swipe: Swipe) {
        if let Err(error) = self.0.emit(SWIPED, &swipe) {
            tracing::debug!("cannot publish swipe: {error}");
        }
    }
}

#[derive(Debug)]
pub(crate) struct GesturesService;

impl Service<Compositor> for GesturesService {
    fn manifest(&self) -> Manifest {
        Manifest::new(CAPABILITY, VERSION)
            .summary("Touchpad swipes reserved for the shell")
            .event(EventInfo::new(SWIPED).summary("Swipe began, updated or ended"))
    }

    fn call(&mut self, _: &mut Compositor, _: Method, call: Call) {
        call.unknown_method();
    }
}
