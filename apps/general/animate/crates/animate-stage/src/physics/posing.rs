use rikka_physics::{LiveInputs, LivePose};
use rikka_physics_tailor::Wind;

use super::cloth_assets::ClothAssets;
use crate::actor::Actor;
use crate::holding::{self, Performer};

pub(super) struct Posing<'a> {
    pub(super) actors: &'a mut [Actor],
    pub(super) assets: &'a ClothAssets,
    pub(super) wind: Wind,
    pub(super) posed: f32,
}

impl Posing<'_> {
    pub(super) fn animate_to(&mut self, time: f32) {
        if self.posed != time {
            animate_actors(self.actors, time);
            self.posed = time;
        }
    }
}

impl LivePose for Posing<'_> {
    fn pose(&mut self, time: f32) -> LiveInputs<'_> {
        self.animate_to(time);
        let grabs: Vec<_> =
            holding::active_grabs(time, &performers(self.actors, self.assets, true))
                .into_iter()
                .map(|active| (self.actors[active.owner].id.slot(), active.grab))
                .collect();
        LiveInputs::new()
            .wind(self.wind)
            .figures(
                self.actors
                    .iter()
                    .map(|actor| (actor.id.slot(), &actor.instance)),
            )
            .grabs(grabs)
    }
}

pub(super) fn animate_actors(actors: &mut [Actor], time: f32) {
    for actor in actors {
        if actor.animate(time) {
            actor.instance.refresh();
        }
    }
}

pub(super) fn performers<'a>(
    actors: &'a [Actor],
    assets: &'a ClothAssets,
    enabled: bool,
) -> Vec<Performer<'a>> {
    actors
        .iter()
        .map(|actor| Performer {
            key: actor.id.number(),
            name: &actor.name,
            instance: &actor.instance,
            pinches: actor.pinches,
            holds: actor.holds(),
            cloth_asset: assets
                .current(actor.id)
                .filter(|_| enabled)
                .map(|asset| &asset.cloth_asset),
        })
        .collect()
}
