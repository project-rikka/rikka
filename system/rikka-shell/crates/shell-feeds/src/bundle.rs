use std::cell::RefCell;
use std::rc::Rc;

use rikka_settings::session::weather::{self, DOMAIN, LOCATION, PLACE, TEMPERATURE_UNIT};
use rikka_weather::{Place, Spot};
use shell_kit::{Shell, WallClock};

use crate::audio_spectrum::AudioSpectrum;
use crate::daemons::Daemons;
use crate::scale::Scale;
use crate::services::Services;

#[derive(Clone, Debug)]
pub struct Feeds {
    pub services: Services,
    pub audio_spectrum: AudioSpectrum,
    pub scale: Scale,
    pub wall: WallClock,
}

#[derive(Debug, Default)]
struct LocationSetting {
    location: String,
    place: Option<Place>,
}

impl LocationSetting {
    fn spot(&self) -> Spot {
        Spot::from_setting(&self.location, self.place.clone())
    }
}

impl Feeds {
    #[must_use]
    pub fn follow(shell: Shell, daemons: Daemons) -> (Shell, Feeds) {
        let shell = shell.settings(weather::schema());
        let whereabouts = LocationSetting {
            location: shell.setting(DOMAIN, LOCATION).unwrap_or_default(),
            place: placed(shell.setting(DOMAIN, PLACE)),
        };
        let feeds = Feeds {
            services: Services::start(shell.name(), daemons, whereabouts.spot()),
            audio_spectrum: AudioSpectrum::default(),
            scale: Scale::default(),
            wall: shell.wall(),
        };

        let whereabouts = Rc::new(RefCell::new(whereabouts));
        let shell = shell
            .on_setting(DOMAIN, LOCATION, {
                let (whereabouts, services) = (Rc::clone(&whereabouts), feeds.services.clone());
                move |_, location: String| {
                    whereabouts.borrow_mut().location = location;
                    services.weather().set_spot(whereabouts.borrow().spot());
                }
            })
            .on_setting(DOMAIN, PLACE, {
                let services = feeds.services.clone();
                move |_, place: serde_json::Value| {
                    whereabouts.borrow_mut().place = placed(Some(place));
                    services.weather().set_spot(whereabouts.borrow().spot());
                }
            })
            .follow(DOMAIN, TEMPERATURE_UNIT, {
                let scale = feeds.scale.clone();
                move |_, unit: String| scale.chosen(&unit)
            });
        (shell, feeds)
    }
}

fn placed(value: Option<serde_json::Value>) -> Option<Place> {
    serde_json::from_value(value?).ok()
}
