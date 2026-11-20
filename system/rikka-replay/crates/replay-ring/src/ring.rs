use std::ops::Range;
use std::time::Duration;

use crate::clip::Clip;
use crate::track::Track;
use rikka_coded::Coded;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PictureId(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub from_us: i64,
    pub to_us: i64,
    pub ended: bool,
}

#[derive(Debug)]
struct PictureTrack {
    id: PictureId,
    track: Track,
    ended: bool,
}

#[derive(Debug)]
pub struct Ring {
    window_us: i64,
    budget: usize,
    pictures: Vec<PictureTrack>,
    sounds: Vec<Track>,
    next_id: u64,
}

impl Ring {
    #[must_use]
    pub fn new(window: Duration, budget: usize, sounds: usize) -> Ring {
        Ring {
            window_us: micros(window),
            budget,
            pictures: Vec::new(),
            sounds: (0..sounds).map(|_| Track::default()).collect(),
            next_id: 0,
        }
    }

    pub fn set_limits(&mut self, window: Duration, budget: usize) {
        self.window_us = micros(window);
        self.budget = budget;
    }

    #[must_use]
    pub fn window(&self) -> Duration {
        Duration::from_micros(self.window_us.max(0) as u64)
    }

    #[must_use]
    pub fn budget(&self) -> usize {
        self.budget
    }

    pub fn open_pictures(&mut self) -> PictureId {
        self.next_id += 1;
        let id = PictureId(self.next_id);
        self.pictures.push(PictureTrack {
            id,
            track: Track::default(),
            ended: false,
        });
        id
    }

    pub fn end_pictures(&mut self, id: PictureId) {
        if let Some(pictures) = self.find_mut(id) {
            pictures.ended = true;
        }
    }

    fn forget_pictures(&mut self, id: PictureId) {
        self.pictures.retain(|pictures| pictures.id != id);
    }

    pub fn push_picture(&mut self, id: PictureId, packet: Coded) {
        if let Some(pictures) = self.find_mut(id) {
            pictures.track.push_picture(packet);
        }
    }

    pub fn push_sound(&mut self, track: usize, packet: Coded) {
        if let Some(sound) = self.sounds.get_mut(track) {
            sound.push(packet);
        }
    }

    pub fn clear_sound(&mut self, track: usize) {
        if let Some(sound) = self.sounds.get_mut(track) {
            sound.clear();
        }
    }

    pub fn trim(&mut self, now_us: i64) {
        let cutoff = now_us - self.window_us;
        self.pictures.retain(|pictures| {
            !pictures.ended || pictures.track.last_end().is_some_and(|end| end > cutoff)
        });
        for pictures in &mut self.pictures {
            if let Some(start) = pictures.track.keyframe_at_or_before(cutoff) {
                pictures.track.drop_front(start);
            }
        }
        self.trim_sounds(cutoff);
        while self.used() > self.budget {
            if !self.drop_oldest_stretch() {
                break;
            }
            self.trim_sounds(cutoff);
        }
    }

    #[must_use]
    pub fn used(&self) -> usize {
        self.pictures
            .iter()
            .map(|pictures| pictures.track.byte_size())
            .chain(self.sounds.iter().map(Track::byte_size))
            .sum()
    }

    pub fn pictures(&self) -> impl Iterator<Item = PictureId> + '_ {
        self.pictures.iter().map(|pictures| pictures.id)
    }

    #[must_use]
    pub fn span(&self, id: PictureId) -> Option<Span> {
        let pictures = self.find(id)?;
        Some(Span {
            from_us: pictures.track.first_pts()?,
            to_us: pictures.track.last_end()?,
            ended: pictures.ended,
        })
    }

    #[must_use]
    pub fn clip(&self, id: PictureId, span: Range<i64>) -> Option<Clip> {
        let pictures = self.find(id)?;
        Clip::cut(&pictures.track, &self.sounds, span)
    }

    fn find(&self, id: PictureId) -> Option<&PictureTrack> {
        self.pictures.iter().find(|pictures| pictures.id == id)
    }

    fn find_mut(&mut self, id: PictureId) -> Option<&mut PictureTrack> {
        self.pictures.iter_mut().find(|pictures| pictures.id == id)
    }

    fn drop_oldest_stretch(&mut self) -> bool {
        let oldest = self
            .pictures
            .iter_mut()
            .filter_map(|pictures| {
                let next = pictures.track.second_keyframe()?;
                Some((pictures.track.first_pts()?, next, &mut pictures.track))
            })
            .min_by_key(|(first, _, _)| *first);
        if let Some((_, next, track)) = oldest {
            track.drop_front(next);
            return true;
        }
        let ended = self
            .pictures
            .iter()
            .filter(|pictures| pictures.ended)
            .min_by_key(|pictures| pictures.track.first_pts())
            .map(|pictures| pictures.id);
        match ended {
            Some(id) => {
                self.forget_pictures(id);
                true
            }
            None => false,
        }
    }

    fn trim_sounds(&mut self, cutoff: i64) {
        let earliest = self
            .pictures
            .iter()
            .filter_map(|pictures| pictures.track.first_pts())
            .min()
            .unwrap_or(cutoff);
        let floor = cutoff.max(earliest);
        for sound in &mut self.sounds {
            sound.drop_before_pts(floor);
        }
    }
}

fn micros(duration: Duration) -> i64 {
    duration.as_micros().min(i64::MAX as u128) as i64
}
