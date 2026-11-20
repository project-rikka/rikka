use std::time::Duration;

use rikka_compositor_shell::{ActivationToken, TokenRequest};
use rikka_compositor_surface::SurfaceId;

use crate::context::WindowsCtx;

pub(crate) const LIFETIME: Duration = Duration::from_secs(10);

impl WindowsCtx<'_> {
    pub(crate) fn admits_activation(&mut self, request: &TokenRequest) -> bool {
        self.forget_stale_tokens();
        let Some((serial, seat)) = &request.serial else {
            return false;
        };
        let person = self.input.seat;
        self.seats.seat_id(seat) == Some(person)
            && self
                .seats
                .get(person)
                .is_some_and(|seat_state| seat_state.keyboard().issued_since_enter(*serial))
    }

    pub(crate) fn forget_stale_tokens(&mut self) {
        self.shell
            .retain_activation_tokens(|token| token.created.elapsed() < LIFETIME);
    }

    pub(crate) fn mint_activation_token(
        &mut self,
        app_id: Option<String>,
    ) -> rikka_session::activation::ActivationToken {
        self.forget_stale_tokens();
        let token = self.shell.mint_activation_token(app_id);
        rikka_session::activation::ActivationToken { token }
    }

    pub(crate) fn activate(&mut self, token: ActivationToken, surface: SurfaceId) {
        if token.created.elapsed() > LIFETIME {
            return tracing::debug!("ignoring expired activation token");
        }
        if let Some(window) = self.windows.window_of(self.surfaces, surface) {
            self.reveal_window(window);
            self.windows.session_changed();
        }
    }
}
