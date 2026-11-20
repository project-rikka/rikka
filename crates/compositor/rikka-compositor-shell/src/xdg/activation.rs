use std::time::Instant;

use rikka_compositor_protocols::wl_seat::WlSeat;
use rikka_compositor_protocols::xdg_activation_token_v1::{self, Error, XdgActivationTokenV1};
use rikka_compositor_protocols::xdg_activation_v1::{self, XdgActivationV1};
use rikka_compositor_surface::SurfaceId;
use rikka_wayland_wire::ClientId;

use crate::event::ShellEvent;
use crate::host::ShellHost;
use crate::ids::ActivationDraftId;
use crate::shell::Shell;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TokenRequest {
    pub client: Option<ClientId>,
    pub serial: Option<(u32, WlSeat)>,
    pub app_id: Option<String>,
    pub surface: Option<SurfaceId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivationToken {
    pub token: String,
    pub request: TokenRequest,
    pub created: Instant,
}

#[derive(Debug)]
pub(crate) struct Draft {
    request: TokenRequest,
    committed: bool,
}

impl Shell {
    pub fn mint_activation_token(&mut self, app_id: Option<String>) -> String {
        self.remember_token(TokenRequest {
            app_id,
            ..TokenRequest::default()
        })
    }

    pub fn retain_activation_tokens(&mut self, mut keep: impl FnMut(&ActivationToken) -> bool) {
        self.tokens.retain(|_, token| keep(token));
    }

    fn remember_token(&mut self, request: TokenRequest) -> String {
        let token = rikka_compositor_types::unguessable();
        self.tokens.insert(
            token.clone(),
            ActivationToken {
                token: token.clone(),
                request,
                created: Instant::now(),
            },
        );
        token
    }
}

pub(crate) fn manager<S: ShellHost>(
    state: &mut S,
    _activation: XdgActivationV1,
    request: xdg_activation_v1::Request,
) {
    match request {
        xdg_activation_v1::Request::Destroy => {}
        xdg_activation_v1::Request::GetActivationToken { id } => {
            let client = id.client();
            let draft = state.shell().activations.insert(Draft {
                request: TokenRequest {
                    client: Some(client),
                    ..TokenRequest::default()
                },
                committed: false,
            });
            id.set_key(draft);
        }
        xdg_activation_v1::Request::Activate { token, surface } => {
            let Some(surface) = state.surfaces().lookup(&surface) else {
                return;
            };
            let Some(token) = state.shell().tokens.remove(token) else {
                return;
            };
            state.shell_event(ShellEvent::Activate { token, surface });
        }
    }
}

pub(crate) fn request<S: ShellHost>(
    state: &mut S,
    token: XdgActivationTokenV1,
    request: xdg_activation_token_v1::Request,
) {
    if matches!(request, xdg_activation_token_v1::Request::Destroy) {
        return;
    }
    let Some(key) = token.key::<ActivationDraftId>() else {
        return;
    };
    let surface = match &request {
        xdg_activation_token_v1::Request::SetSurface { surface } => {
            state.surfaces().lookup(surface)
        }
        _ => None,
    };
    let shell = state.shell();
    let Some(draft) = shell.activations.get_mut(key) else {
        return;
    };
    if draft.committed {
        return token.post_error(Error::AlreadyUsed);
    }
    match request {
        xdg_activation_token_v1::Request::Destroy => {}
        xdg_activation_token_v1::Request::SetSerial { serial, seat } => {
            draft.request.serial = Some((serial, seat));
        }
        xdg_activation_token_v1::Request::SetAppId { app_id } => {
            draft.request.app_id = Some(app_id.to_owned());
        }
        xdg_activation_token_v1::Request::SetSurface { .. } => draft.request.surface = surface,
        xdg_activation_token_v1::Request::Commit => {
            draft.committed = true;
            let request = draft.request.clone();
            let issued = if state.admit_activation(&request) {
                state.shell().remember_token(request)
            } else {
                rikka_compositor_types::unguessable()
            };
            token.done(&issued);
        }
    }
}

pub(crate) fn destroyed<S: ShellHost>(state: &mut S, token: XdgActivationTokenV1) {
    if let Some(key) = token.key::<ActivationDraftId>() {
        state.shell().activations.remove(key);
    }
}
