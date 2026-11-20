use std::collections::HashMap;
use std::time::{Duration, Instant};

use discord_core::{UserId, VideoFrame, VoiceEvent};
use discord_rtp::{Arrived, Feedback, Header, Payload, VideoReceiver};

use crate::dave::{Dave, Reply};
use crate::error::Result;
use crate::gateway::Video;

#[derive(Debug, Default)]
pub(crate) struct Watching {
    receiver: VideoReceiver,
    owners: HashMap<u32, UserId>,
}

impl Watching {
    pub(crate) fn announce(&mut self, video: &Video) -> Vec<VoiceEvent> {
        let mut events = Vec::new();
        let active: Vec<u32> = video.streams.iter().map(|stream| stream.ssrc).collect();
        let stale: Vec<u32> = self
            .owners
            .iter()
            .filter(|(ssrc, owner)| **owner == video.user && !active.contains(ssrc))
            .map(|(&ssrc, _)| ssrc)
            .collect();
        for ssrc in stale {
            self.owners.remove(&ssrc);
            self.receiver.forget(ssrc);
            events.push(VoiceEvent::VideoGone { source: ssrc });
        }
        for stream in &video.streams {
            self.receiver.announce(stream.ssrc, stream.rtx_ssrc);
            if self.owners.insert(stream.ssrc, video.user) != Some(video.user) {
                events.push(VoiceEvent::VideoSource {
                    source: stream.ssrc,
                    user: video.user,
                });
            }
        }
        events
    }

    pub(crate) fn left(&mut self, user: UserId) -> Vec<VoiceEvent> {
        self.announce(&Video {
            user,
            audio: 0,
            streams: Vec::new(),
        })
    }

    pub(crate) fn sources(&self) -> Vec<u32> {
        let mut sources: Vec<u32> = self.owners.keys().copied().collect();
        sources.sort_unstable();
        sources
    }

    pub(crate) fn known(&self) -> Vec<(u32, UserId)> {
        let mut known: Vec<(u32, UserId)> = self
            .owners
            .iter()
            .map(|(&ssrc, &user)| (ssrc, user))
            .collect();
        known.sort_unstable();
        known
    }

    pub(crate) fn packet(
        &mut self,
        header: Header,
        kind: Payload,
        payload: Vec<u8>,
        dave: Option<&mut Dave>,
        replies: &mut Vec<Reply>,
        now: Instant,
    ) -> Result<Vec<VoiceEvent>> {
        let arrived = self.receiver.packet(header, kind, payload, now);
        let mut events = Vec::with_capacity(arrived.len());
        let mut dave = dave;
        for Arrived {
            source,
            codec,
            frame,
        } in arrived
        {
            let sender = self.owners.get(&source).map(|user| user.get());
            let data = match dave.as_deref_mut() {
                None => Some(frame.data),
                Some(dave) => dave
                    .decrypt_video(sender, &frame.data, replies)?
                    .map(std::borrow::Cow::into_owned),
            };
            let Some(data) = data else {
                self.receiver.broke(source);
                continue;
            };
            events.push(VoiceEvent::Video(VideoFrame {
                source,
                codec,
                timestamp: frame.timestamp,
                keyframe: frame.keyframe,
                data,
            }));
        }
        Ok(events)
    }

    pub(crate) fn broke(&mut self, source: u32) {
        self.receiver.broke(source);
    }

    pub(crate) fn on_feedback(&mut self, feedback: &Feedback, now: Instant) {
        if let Feedback::Sender { source, ntp } = feedback {
            self.receiver.sender_reported(*source, *ntp, now);
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.receiver.is_empty()
    }

    pub(crate) fn feedback(&mut self, me: u32, now: Instant, round_trip: Duration) -> Vec<Vec<u8>> {
        self.receiver.feedback(me, now, round_trip)
    }

    pub(crate) fn report(&mut self, me: u32, now: Instant) -> Option<Vec<u8>> {
        self.receiver.report(me, now)
    }
}
