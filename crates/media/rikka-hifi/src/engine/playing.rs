use rikka_hifi_core::{Pipeline, Plan, ReplayGain};

use crate::source::{SourceId, SourceStream};

pub(crate) struct Playing {
    pub(crate) stream: SourceStream,
    pub(crate) pipeline: Pipeline,
    pub(crate) finished: bool,
}

impl Playing {
    pub(crate) fn new(
        stream: SourceStream,
        plan: Plan,
        volume: Option<f32>,
        replay_gain: &ReplayGain,
    ) -> Playing {
        let mut playing = Playing {
            pipeline: Pipeline::new(&stream.format, plan),
            stream,
            finished: false,
        };
        playing.update_gains(volume, replay_gain);
        playing
    }

    pub(crate) fn id(&self) -> SourceId {
        self.stream.id
    }

    pub(crate) fn plan(&self) -> Plan {
        *self.pipeline.plan()
    }

    pub(crate) fn update_gains(&mut self, volume: Option<f32>, replay_gain: &ReplayGain) {
        self.pipeline
            .set_replay_gain(replay_gain.applied_db(&self.stream.gains));
        self.pipeline.set_volume(volume);
    }

    pub(crate) fn set_plan(&mut self, plan: Plan, volume: Option<f32>, replay_gain: &ReplayGain) {
        self.pipeline = Pipeline::new(&self.stream.format, plan);
        self.finished = false;
        self.update_gains(volume, replay_gain);
    }
}
