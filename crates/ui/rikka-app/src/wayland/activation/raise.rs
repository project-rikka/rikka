use rikka_wayland::{Host, SerialKind, Serials};
use rikka_wayland_client::wl_surface::WlSurface;
use rikka_wayland_client::xdg_activation_token_v1::{self, XdgActivationTokenV1};
use rikka_wayland_client::xdg_activation_v1::XdgActivationV1;
use rikka_wayland_client::{Connection, Route};

const ACTING: [SerialKind; 3] = [
    SerialKind::ButtonPressed,
    SerialKind::KeyPressed,
    SerialKind::KeyboardEntered,
];

#[must_use]
#[derive(Debug)]
pub(crate) struct Raise {
    connection: Connection,
    activation: XdgActivationV1,
    serials: Serials,
}

impl Raise {
    pub(crate) fn on(host: &Host) -> Option<Raise> {
        let serials = host
            .serials()
            .inspect_err(|error| tracing::debug!("no seat available: {error}"))
            .ok()?;
        let activation = host
            .display()
            .bind(1..=1)
            .inspect_err(|error| tracing::debug!("xdg-activation unsupported: {error}"))
            .ok()?;
        Some(Raise {
            connection: host.connection().clone(),
            activation,
            serials,
        })
    }

    pub(crate) fn forward(&self, surface: &WlSurface, token: &str) {
        self.activation.activate(token, surface);
        flush(&self.connection);
    }

    pub(crate) fn transfer_focus(&self, from: &WlSurface, to: &WlSurface, app_id: &str) {
        let token = self
            .activation
            .via(&self.granted_to(to.clone()))
            .get_activation_token();
        match (self.serials.latest(&ACTING), self.serials.seat()) {
            (Some(acted), Some(seat)) => token.set_serial(acted.serial, &seat),
            _ => tracing::debug!("no input serial for activation"),
        }
        token.set_surface(from);
        token.set_app_id(app_id);
        token.commit();
        flush(&self.connection);
    }

    fn granted_to(&self, target: WlSurface) -> Route {
        let activation = self.activation.clone();
        let connection = self.connection.clone();
        Route::tap::<XdgActivationTokenV1>(move |token, event| {
            if let xdg_activation_token_v1::Event::Done { token: granted } = event {
                activation.activate(&granted, &target);
                token.destroy();
                flush(&connection);
            }
        })
    }
}

fn flush(connection: &Connection) {
    if let Err(error) = connection.flush() {
        tracing::debug!("failed to flush activation request: {error}");
    }
}
