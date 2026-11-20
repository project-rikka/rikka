use rikka_seam::{Call, Manifest, Method, MethodInfo, Service};
use rikka_session::activation::{ActivationRequest, CAPABILITY, MINT, VERSION};

use crate::context::windows_ctx;
use crate::state::Compositor;

#[derive(Debug)]
pub(crate) struct ActivationService;

impl Service<Compositor> for ActivationService {
    fn manifest(&self) -> Manifest {
        Manifest::new(CAPABILITY, VERSION)
            .summary("Mints XDG activation tokens")
            .method(
                MethodInfo::new(MINT).summary("Returns a short-lived, single-use activation token"),
            )
    }

    fn call(&mut self, compositor: &mut Compositor, method: Method, call: Call) {
        match &*method {
            MINT => {
                let request = match call.payload().is_none() {
                    true => ActivationRequest::default(),
                    false => match call.decode::<ActivationRequest>() {
                        Ok(request) => request,
                        Err(error) => return call.fail(error),
                    },
                };
                call.encode(&windows_ctx!(compositor).mint_activation_token(request.app_id));
            }
            _ => call.unknown_method(),
        }
    }
}
