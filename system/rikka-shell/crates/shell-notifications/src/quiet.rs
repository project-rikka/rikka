use futures_util::Stream;
use rikka_seam::SeamClient;
use rikka_settings::seam::protocol;
use rikka_worker::Observable;

use crate::settings::{DO_NOT_DISTURB, DOMAIN};

#[derive(Clone, Debug, Default)]
pub struct Quiet(Observable<bool>);

impl Quiet {
    #[must_use]
    pub fn get(&self) -> bool {
        self.0.get()
    }

    pub(crate) fn set(&self, quiet: bool) {
        self.0.update(quiet);
    }

    pub fn ask(&self, seam: &SeamClient, quiet: bool) {
        self.set(quiet);
        seam.notify(protocol::set(DOMAIN, DO_NOT_DISTURB, quiet));
    }

    pub fn changes(&self) -> impl Stream<Item = bool> + Send + 'static {
        self.0.changes()
    }
}
