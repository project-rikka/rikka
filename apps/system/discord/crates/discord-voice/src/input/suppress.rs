use std::sync::mpsc;

use discord_krisp::{NoiseCleaner, Sdk, VoiceDetector};

use crate::krisp::Krisp;
use crate::settings::NoiseSuppression;

const OPENER: &str = "rikka-discord-krisp-open";

#[derive(Debug)]
pub(crate) struct Suppress {
    krisp: Krisp,
    mode: NoiseSuppression,
    sessions: Sessions,
}

#[derive(Debug)]
enum Sessions {
    Waiting,
    Opening(mpsc::Receiver<Option<Box<KrispSessions>>>),
    Open(Box<KrispSessions>),
    Failed,
}

#[derive(Debug)]
struct KrispSessions {
    cleaner: NoiseCleaner,
    detector: Option<VoiceDetector>,
}

impl Suppress {
    pub(crate) fn new(krisp: Krisp, mode: NoiseSuppression) -> Suppress {
        let mut suppress = Suppress {
            krisp,
            mode: NoiseSuppression::Off,
            sessions: Sessions::Waiting,
        };
        suppress.set_mode(mode);
        suppress
    }

    pub(crate) fn set_mode(&mut self, mode: NoiseSuppression) {
        if mode == self.mode {
            return;
        }
        self.mode = mode;
        self.sessions = Sessions::Waiting;
    }

    pub(crate) fn needs_standard(&mut self) -> bool {
        match self.mode {
            NoiseSuppression::Off => false,
            NoiseSuppression::Standard => true,
            NoiseSuppression::Krisp => !self.ready(),
        }
    }

    pub(crate) fn clean(&mut self, frame: &mut [f32; discord_krisp::FRAME]) -> Option<f32> {
        if self.mode != NoiseSuppression::Krisp || !self.ready() {
            return None;
        }
        let Sessions::Open(open_sessions) = &mut self.sessions else {
            return None;
        };
        if let Err(error) = open_sessions.cleaner.process(frame) {
            tracing::warn!("Krisp noise cancellation failed: {error}");
            self.sessions = Sessions::Failed;
            return None;
        }
        let detector = open_sessions.detector.as_mut()?;
        match detector.probability(frame) {
            Ok(probability) => Some(probability),
            Err(error) => {
                tracing::warn!("Krisp VAD failed: {error}");
                open_sessions.detector = None;
                None
            }
        }
    }

    fn ready(&mut self) -> bool {
        match &self.sessions {
            Sessions::Open(_) => true,
            Sessions::Failed => false,
            Sessions::Waiting => {
                if let Some(sdk) = self.krisp.sdk() {
                    self.sessions = open(sdk.clone());
                }
                false
            }
            Sessions::Opening(arriving) => match arriving.try_recv() {
                Ok(Some(open_sessions)) => {
                    self.sessions = Sessions::Open(open_sessions);
                    true
                }
                Ok(None) | Err(mpsc::TryRecvError::Disconnected) => {
                    self.sessions = Sessions::Failed;
                    false
                }
                Err(mpsc::TryRecvError::Empty) => false,
            },
        }
    }
}

fn open(sdk: Sdk) -> Sessions {
    let (sender, receiver) = mpsc::channel();
    let started = std::thread::Builder::new()
        .name(OPENER.into())
        .spawn(move || {
            let open_sessions = sdk
                .noise_cleaner()
                .inspect_err(|error| {
                    tracing::warn!("failed to open Krisp noise cancellation: {error}")
                })
                .ok()
                .map(|cleaner| {
                    Box::new(KrispSessions {
                        cleaner,
                        detector: sdk
                            .voice_detector()
                            .inspect_err(|error| {
                                tracing::warn!("failed to open Krisp VAD: {error}")
                            })
                            .ok(),
                    })
                });
            let _ = sender.send(open_sessions);
        });
    match started {
        Ok(_) => Sessions::Opening(receiver),
        Err(error) => {
            tracing::warn!("failed to spawn Krisp thread: {error}");
            Sessions::Failed
        }
    }
}
