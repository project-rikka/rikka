use std::time::Duration;

use rikka_hifi_core::ReplayGain;
use rikka_worker::Subscriber;

use crate::event::Event;
use crate::output::Output;
use crate::source::{Source, SourceId};

pub(crate) enum Command {
    Subscribe(Subscriber<Event>),
    Output(Output),
    Play(SourceId, Source),
    Queue(SourceId, Source),
    Unqueue,
    Pause,
    Resume,
    Stop,
    Seek(Duration),
    Volume(f32),
    ReplayGain(ReplayGain),
    Quit,
}
