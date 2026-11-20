#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Loudness {
    pub gain_db: f32,
    pub peak: Option<f32>,
}

impl Loudness {
    #[must_use]
    pub fn new(gain_db: f32) -> Loudness {
        Loudness {
            gain_db,
            peak: None,
        }
    }

    #[must_use]
    pub fn peak(mut self, peak: f32) -> Loudness {
        self.peak = Some(peak);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Gains {
    pub track: Option<Loudness>,
    pub album: Option<Loudness>,
}

impl Gains {
    #[must_use]
    pub fn track(mut self, loudness: Loudness) -> Gains {
        self.track = Some(loudness);
        self
    }

    #[must_use]
    pub fn album(mut self, loudness: Loudness) -> Gains {
        self.album = Some(loudness);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ReplayGainMode {
    #[default]
    Off,
    Track,
    Album,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReplayGain {
    pub mode: ReplayGainMode,
    pub preamp_db: f32,
    pub prevent_clipping: bool,
}

impl Default for ReplayGain {
    fn default() -> ReplayGain {
        ReplayGain::new(ReplayGainMode::Off)
    }
}

impl ReplayGain {
    #[must_use]
    pub fn new(mode: ReplayGainMode) -> ReplayGain {
        ReplayGain {
            mode,
            preamp_db: 0.0,
            prevent_clipping: true,
        }
    }

    #[must_use]
    pub fn preamp(mut self, db: f32) -> ReplayGain {
        self.preamp_db = db;
        self
    }

    #[must_use]
    pub fn prevent_clipping(mut self, prevent: bool) -> ReplayGain {
        self.prevent_clipping = prevent;
        self
    }

    #[must_use]
    pub fn applied_db(&self, gains: &Gains) -> f32 {
        let chosen = match self.mode {
            ReplayGainMode::Off => return 0.0,
            ReplayGainMode::Track => gains.track.or(gains.album),
            ReplayGainMode::Album => gains.album.or(gains.track),
        };
        let Some(loudness) = chosen else {
            return 0.0;
        };
        let gain_db = loudness.gain_db + self.preamp_db;
        match loudness.peak.filter(|peak| *peak > 0.0) {
            Some(peak) if self.prevent_clipping => gain_db.min(-20.0 * peak.log10()),
            _ => gain_db,
        }
    }
}

#[must_use]
pub fn amplitude(db: f32) -> f32 {
    if db == 0.0 {
        return 1.0;
    }
    10.0_f32.powf(db / 20.0)
}
