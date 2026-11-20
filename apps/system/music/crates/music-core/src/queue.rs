use crate::item::Track;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Repeat {
    #[default]
    Off,
    All,
    One,
}

impl Repeat {
    #[must_use]
    pub fn next(self) -> Repeat {
        match self {
            Repeat::Off => Repeat::All,
            Repeat::All => Repeat::One,
            Repeat::One => Repeat::Off,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Queue {
    tracks: Vec<Track>,
    order: Vec<usize>,
    at: usize,
    shuffle: bool,
    repeat: Repeat,
    seed: u64,
}

impl Queue {
    #[must_use]
    pub fn new(seed: u64) -> Queue {
        Queue {
            seed: seed | 1,
            ..Queue::default()
        }
    }

    pub fn start(&mut self, tracks: Vec<Track>, from: usize) -> Option<&Track> {
        let skipped = tracks
            .iter()
            .take(from)
            .filter(|track| !track.playable)
            .count();
        self.tracks = tracks.into_iter().filter(|track| track.playable).collect();
        self.order = (0..self.tracks.len()).collect();
        self.at = (from - skipped).min(self.tracks.len().saturating_sub(1));
        if self.shuffle {
            self.shuffled();
        }
        self.current()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.order.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    #[must_use]
    pub fn current(&self) -> Option<&Track> {
        self.order.get(self.at).map(|index| &self.tracks[*index])
    }

    #[must_use]
    pub fn shuffle(&self) -> bool {
        self.shuffle
    }

    #[must_use]
    pub fn repeat(&self) -> Repeat {
        self.repeat
    }

    pub fn set_repeat(&mut self, repeat: Repeat) {
        self.repeat = repeat;
    }

    pub fn set_shuffle(&mut self, shuffle: bool) {
        if self.shuffle == shuffle {
            return;
        }
        self.shuffle = shuffle;
        match shuffle {
            true => self.shuffled(),
            false => {
                let playing = self.order.get(self.at).copied();
                self.order = (0..self.tracks.len()).collect();
                self.at = playing.unwrap_or(0);
            }
        }
    }

    pub fn advance(&mut self, ended: bool) -> Option<&Track> {
        self.at = self.after(ended)?;
        self.current()
    }

    #[must_use]
    pub fn following(&self) -> Option<&Track> {
        self.after(true).map(|at| &self.tracks[self.order[at]])
    }

    fn after(&self, ended: bool) -> Option<usize> {
        if self.order.is_empty() {
            return None;
        }
        if ended && self.repeat == Repeat::One {
            return Some(self.at);
        }
        if self.at + 1 < self.order.len() {
            return Some(self.at + 1);
        }
        match self.repeat {
            Repeat::Off => None,
            Repeat::All | Repeat::One => Some(0),
        }
    }

    pub fn back(&mut self) -> Option<&Track> {
        if self.at > 0 {
            self.at -= 1;
        } else if self.repeat != Repeat::Off && self.order.len() > 1 {
            self.at = self.order.len() - 1;
        } else {
            return None;
        }
        self.current()
    }

    pub fn upcoming(&self) -> impl Iterator<Item = &Track> {
        self.order
            .iter()
            .skip(self.at + 1)
            .map(|index| &self.tracks[*index])
    }

    pub fn skip_to(&mut self, ahead: usize) -> Option<&Track> {
        let at = self.at + 1 + ahead;
        if at >= self.order.len() {
            return None;
        }
        self.at = at;
        self.current()
    }

    pub fn enqueue(&mut self, track: Track) {
        self.tracks.push(track);
        self.order.push(self.tracks.len() - 1);
    }

    pub fn play_next(&mut self, track: Track) {
        let Some(playing) = self.order.get(self.at).copied() else {
            self.enqueue(track);
            return;
        };
        let slot = playing + 1;
        self.tracks.insert(slot, track);
        for index in &mut self.order {
            if *index >= slot {
                *index += 1;
            }
        }
        self.order.insert(self.at + 1, slot);
    }

    pub fn remove_upcoming(&mut self, ahead: usize) -> Option<Track> {
        let at = self.at + 1 + ahead;
        if at >= self.order.len() {
            return None;
        }
        let slot = self.order.remove(at);
        for index in &mut self.order {
            if *index > slot {
                *index -= 1;
            }
        }
        Some(self.tracks.remove(slot))
    }

    fn shuffled(&mut self) {
        let Some(playing) = self.order.get(self.at).copied() else {
            return;
        };
        let mut rest: Vec<usize> = (0..self.tracks.len())
            .filter(|index| *index != playing)
            .collect();
        for last in (1..rest.len()).rev() {
            let pick = (self.roll() % (last as u64 + 1)) as usize;
            rest.swap(last, pick);
        }
        self.order = std::iter::once(playing).chain(rest).collect();
        self.at = 0;
    }

    fn roll(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
}
