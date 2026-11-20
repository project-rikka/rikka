use std::cell::Cell;

use alsa::Round;
use alsa::mixer::{MilliBel, Mixer, Selem, SelemChannelId, SelemId};

const PREFERRED: [&str; 6] = ["PCM", "Master", "Digital", "Speaker", "Headphone", "Front"];

pub(crate) struct HardwareVolume {
    mixer: Mixer,
    name: String,
    index: u32,
    lowest: MilliBel,
    highest: MilliBel,
    found: Option<i64>,
    switched: Option<i32>,
    changed: Cell<bool>,
}

impl HardwareVolume {
    pub(crate) fn find(card: u32) -> Option<HardwareVolume> {
        let mixer = Mixer::new(&format!("hw:{card}"), false).ok()?;
        let mut candidates: Vec<(String, u32, MilliBel, MilliBel)> = mixer
            .iter()
            .filter_map(Selem::new)
            .filter(|selem| selem.has_playback_volume())
            .filter_map(|selem| {
                let (lowest, highest) = selem.get_playback_db_range();
                let id = selem.get_id();
                let name = id.get_name().ok()?.to_owned();
                (highest > lowest).then_some((name, id.get_index(), lowest, highest))
            })
            .collect();
        let rank = |name: &str| {
            PREFERRED
                .iter()
                .position(|preferred| *preferred == name)
                .unwrap_or(PREFERRED.len())
        };
        candidates.sort_by_key(|(name, index, _, _)| (rank(name), *index));
        let (name, index, lowest, highest) = candidates.into_iter().next()?;
        let (found, switched) = {
            let selem = mixer.find_selem(&SelemId::new(&name, index))?;
            (
                selem.get_playback_volume(SelemChannelId::mono()).ok(),
                selem
                    .has_playback_switch()
                    .then(|| selem.get_playback_switch(SelemChannelId::mono()).ok())
                    .flatten(),
            )
        };
        Some(HardwareVolume {
            mixer,
            name,
            index,
            lowest,
            highest,
            found,
            switched,
            changed: Cell::new(false),
        })
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn set(&self, level: f32) {
        let Some(selem) = self.selem() else {
            return;
        };
        self.changed.set(true);
        let target = if level <= 0.0 {
            self.lowest
        } else {
            MilliBel::from_db(20.0 * level.min(1.0).log10())
        };
        let clamped = MilliBel(target.0.clamp(self.lowest.0, self.highest.0));
        if let Err(error) = selem.set_playback_db_all(clamped, Round::Floor) {
            tracing::debug!("failed to set {} mixer to {level}: {error}", self.name);
        }
        if selem.has_playback_switch() {
            let _ = selem.set_playback_switch_all(i32::from(level > 0.0));
        }
    }

    fn selem(&self) -> Option<Selem<'_>> {
        self.mixer.find_selem(&SelemId::new(&self.name, self.index))
    }
}

impl Drop for HardwareVolume {
    fn drop(&mut self) {
        if !self.changed.get() {
            return;
        }
        let Some(selem) = self.selem() else {
            return;
        };
        if let Some(found) = self.found {
            let _ = selem.set_playback_volume_all(found);
        }
        if let Some(switched) = self.switched {
            let _ = selem.set_playback_switch_all(switched);
        }
    }
}

impl std::fmt::Debug for HardwareVolume {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HardwareVolume")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}
