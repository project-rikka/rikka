use rikka_gecko_protocol::Target;
use rikka_gecko_protocol::geckoview::media::{
    MuteAudio, NextTrack, Pause, Play, PreviousTrack, SeekBackward, SeekForward, SeekTo, SkipAd,
    StopMedia,
};

use crate::wire::Link;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MediaCommand {
    Play,
    Pause,
    Stop,
    NextTrack,
    PreviousTrack,
    SeekForward,
    SeekBackward,
    SkipAd,
    SeekTo { seconds: f64, fast: bool },
    Mute(bool),
}

impl MediaCommand {
    pub(crate) fn send(self, link: &Link, target: Target) {
        match self {
            MediaCommand::Play => link.send(target, &Play),
            MediaCommand::Pause => link.send(target, &Pause),
            MediaCommand::Stop => link.send(target, &StopMedia),
            MediaCommand::NextTrack => link.send(target, &NextTrack),
            MediaCommand::PreviousTrack => link.send(target, &PreviousTrack),
            MediaCommand::SeekForward => link.send(target, &SeekForward),
            MediaCommand::SeekBackward => link.send(target, &SeekBackward),
            MediaCommand::SkipAd => link.send(target, &SkipAd),
            MediaCommand::SeekTo { seconds, fast } => link.send(
                target,
                &SeekTo {
                    time: seconds,
                    fast,
                },
            ),
            MediaCommand::Mute(mute) => link.send(target, &MuteAudio { mute }),
        }
    }
}
