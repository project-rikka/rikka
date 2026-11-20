mod peak_buffer;

use peak_buffer::PeakBuffer;

use crate::common::{FRAME_DURATION_MS, MIN_LEVEL_DBFS, VAD_CONFIDENCE_THRESHOLD};

const PEAK_ENVELOPER_SUPER_FRAME_MS: i32 = 400;
const MIN_MARGIN_DB: f32 = 12.0;
const MAX_MARGIN_DB: f32 = 25.0;
const ATTACK: f32 = 0.998_849_4;
const DECAY: f32 = 0.999_769_75;

#[derive(Clone, Copy, Debug)]
struct State {
    headroom_db: f32,
    peak_delay: PeakBuffer,
    max_peaks_dbfs: f32,
    time_since_push_ms: i32,
}

impl State {
    fn new(headroom_db: f32) -> State {
        State {
            headroom_db,
            peak_delay: PeakBuffer::new(),
            max_peaks_dbfs: MIN_LEVEL_DBFS,
            time_since_push_ms: 0,
        }
    }

    fn reset(&mut self, headroom_db: f32) {
        self.headroom_db = headroom_db;
        self.peak_delay.reset();
        self.max_peaks_dbfs = MIN_LEVEL_DBFS;
        self.time_since_push_ms = 0;
    }

    fn update(&mut self, peak_dbfs: f32, speech_level_dbfs: f32) {
        self.max_peaks_dbfs = self.max_peaks_dbfs.max(peak_dbfs);
        self.time_since_push_ms += FRAME_DURATION_MS;
        if self.time_since_push_ms > PEAK_ENVELOPER_SUPER_FRAME_MS {
            self.peak_delay.push(self.max_peaks_dbfs);
            self.max_peaks_dbfs = MIN_LEVEL_DBFS;
            self.time_since_push_ms = 0;
        }
        let delayed_peak_dbfs = self.peak_delay.front().unwrap_or(self.max_peaks_dbfs);
        let difference_db = delayed_peak_dbfs - speech_level_dbfs;
        self.headroom_db = if difference_db > self.headroom_db {
            self.headroom_db * ATTACK + difference_db * (1.0 - ATTACK)
        } else {
            self.headroom_db * DECAY + difference_db * (1.0 - DECAY)
        };
        self.headroom_db = self.headroom_db.clamp(MIN_MARGIN_DB, MAX_MARGIN_DB);
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SaturationProtector {
    initial_headroom_db: f32,
    adjacent_speech_frames_threshold: i32,
    adjacent_speech_frames: i32,
    headroom_db: f32,
    preliminary: State,
    reliable: State,
}

impl SaturationProtector {
    pub(crate) fn new(
        initial_headroom_db: f32,
        adjacent_speech_frames_threshold: i32,
    ) -> SaturationProtector {
        SaturationProtector {
            initial_headroom_db,
            adjacent_speech_frames_threshold,
            adjacent_speech_frames: 0,
            headroom_db: initial_headroom_db,
            preliminary: State::new(initial_headroom_db),
            reliable: State::new(initial_headroom_db),
        }
    }

    pub(crate) fn headroom_db(&self) -> f32 {
        self.headroom_db
    }

    pub(crate) fn reset(&mut self) {
        self.adjacent_speech_frames = 0;
        self.headroom_db = self.initial_headroom_db;
        self.preliminary.reset(self.initial_headroom_db);
        self.reliable.reset(self.initial_headroom_db);
    }

    pub(crate) fn analyze(
        &mut self,
        speech_probability: f32,
        peak_dbfs: f32,
        speech_level_dbfs: f32,
    ) {
        if speech_probability < VAD_CONFIDENCE_THRESHOLD {
            if self.adjacent_speech_frames_threshold > 1 {
                if self.adjacent_speech_frames >= self.adjacent_speech_frames_threshold {
                    self.reliable = self.preliminary;
                } else if self.adjacent_speech_frames > 0 {
                    self.preliminary = self.reliable;
                }
            }
            self.adjacent_speech_frames = 0;
        } else {
            self.adjacent_speech_frames += 1;
            self.preliminary.update(peak_dbfs, speech_level_dbfs);
            if self.adjacent_speech_frames >= self.adjacent_speech_frames_threshold {
                self.headroom_db = self.preliminary.headroom_db;
            }
        }
    }
}
