use rikka_seam::{Call, Manifest, Method, Service, ServiceContext};
use rikka_settings::seam::SettingsService;

use crate::state::Compositor;

pub(super) struct HostedSettings(pub(super) SettingsService);

impl Service<Compositor> for HostedSettings {
    fn manifest(&self) -> Manifest {
        Service::<Compositor>::manifest(&self.0)
    }

    fn attach(&mut self, seam: ServiceContext) {
        Service::<Compositor>::attach(&mut self.0, seam);
    }

    fn call(&mut self, compositor: &mut Compositor, method: Method, call: Call) {
        self.0.call(compositor, method, call);
        compositor.settings_changed();
    }
}
