use futures_util::StreamExt;
use rikka_battery::Battery;
use rikka_power_profiles::PowerProfiles;
use rikka_seam::{Request, SeamClient};
use shell_kit::detached_future;

use crate::service::{CAPABILITY, REVIEW};

const WATCHING: &str = "rikka-power-saving";

pub(super) fn watch(battery: &Battery, profiles: &PowerProfiles, seam: SeamClient) {
    let changes = futures_util::stream::select(
        battery.changes().map(|_| ()),
        profiles.changes().map(|_| ()),
    );
    detached_future(
        WATCHING,
        changes.for_each(move |()| {
            seam.notify(Request::new(CAPABILITY, REVIEW));
            std::future::ready(())
        }),
    );
}
