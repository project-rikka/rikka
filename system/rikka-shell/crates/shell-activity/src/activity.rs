use std::time::Duration;

use rikka_battery::{Charge, Warning};
use rikka_mpris::Player;
use rikka_privacy::{Sensor, User};

#[derive(Clone, Debug, PartialEq)]
pub enum Activity {
    Sensor { sensor: Sensor, users: Vec<User> },
    Media { player: Box<Player> },
    LowBattery { charge: f32, warning: Warning },
    Charging { charge: f32, left: Option<Duration> },
}

pub(super) fn current(
    privacy: &rikka_privacy::State,
    media: &rikka_mpris::State,
    battery: &rikka_battery::State,
) -> Vec<Activity> {
    let mut activities: Vec<Activity> = [Sensor::Screen, Sensor::Camera, Sensor::Microphone]
        .into_iter()
        .filter(|sensor| privacy.in_use(*sensor))
        .map(|sensor| Activity::Sensor {
            sensor,
            users: privacy.users(sensor).to_vec(),
        })
        .collect();
    if let Some(player) = media.active() {
        activities.push(Activity::Media {
            player: Box::new(player.clone()),
        });
    }
    if let Some(main) = battery.main {
        match main.state {
            Charge::Charging => activities.push(Activity::Charging {
                charge: main.charge,
                left: main.left,
            }),
            _ if main.warning > Warning::None && battery.on_battery => {
                activities.push(Activity::LowBattery {
                    charge: main.charge,
                    warning: main.warning,
                });
            }
            _ => {}
        }
    }
    activities
}
