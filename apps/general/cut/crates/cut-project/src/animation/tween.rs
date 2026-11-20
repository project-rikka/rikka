use rikka_keyframe::Easing;

use super::key::{Key, Outgoing, Preset};
use super::kind::Kind;
use super::track::Track;

impl<K: Kind> Track<K> {
    pub(crate) fn tween_preset(&self, last: i64) -> Option<Preset> {
        match self.keys() {
            [start, end] if start.at == 0 && end.at == last => match start.timing.outgoing {
                Outgoing::Preset(preset) => Some(preset),
                Outgoing::Hold | Outgoing::Linear | Outgoing::Bezier(_) => None,
            },
            _ => None,
        }
    }

    pub(crate) fn stretched(&self, last: i64) -> Track<K> {
        let [start, end] = self.keys() else {
            return self.clone();
        };
        Track::from_normalized(vec![*start, Key { at: last, ..*end }])
    }

    pub(crate) fn eased(&self, easing: Easing) -> Track<K> {
        let [start, end] = self.keys() else {
            return self.clone();
        };
        let mut start = *start;
        start.timing.outgoing = Outgoing::Preset(Preset::whole(easing));
        Track::from_normalized(vec![start, *end])
    }

    pub(crate) fn reversed(&self) -> Track<K> {
        let [start, end] = self.keys() else {
            return self.clone();
        };
        let Outgoing::Preset(preset) = start.timing.outgoing else {
            return self.clone();
        };
        let mut first = Key {
            at: start.at,
            value: end.value,
            bend: turned_around::<K>(end.bend),
            ..*start
        };
        first.timing.outgoing = Outgoing::Preset(preset.reversed());
        let second = Key {
            at: end.at,
            value: start.value,
            bend: turned_around::<K>(start.bend),
            ..*end
        };
        Track::from_normalized(vec![first, second])
    }
}

fn turned_around<K: Kind>(mut bend: K::Bend) -> K::Bend {
    if let Some(spatial) = K::bend_mut(&mut bend) {
        *spatial = spatial.reversed();
    }
    bend
}
