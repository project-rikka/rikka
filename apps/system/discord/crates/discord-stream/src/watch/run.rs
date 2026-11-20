use std::task::{Context, Poll};

use discord_client::WatchLine;
use discord_core::{StreamKey, VideoCodec, VideoFrame, VoiceEvent, VoiceEvents};
use discord_voice::StreamSound;
use futures_util::StreamExt as _;
use rikka_video::{LiveCodec, LiveDecoder};
use rikka_worker::CommandReceiver;

use crate::picture::PictureSink;
use crate::update::{Outbox, StreamUpdate};
use crate::watch::Command;

const DEFAULT_FIT: (u32, u32) = (1920, 1080);

struct Watching<'a> {
    key: StreamKey,
    line: &'a WatchLine,
    sound: &'a StreamSound,
    picture: &'a PictureSink,
    outbox: &'a Outbox,
    source: Option<u32>,
    decoder: Option<(VideoCodec, LiveDecoder)>,
    shown: Option<(u32, u32)>,
    fit: (u32, u32),
    failure: Option<String>,
}

pub(crate) fn run(
    key: StreamKey,
    mut events: VoiceEvents,
    line: &WatchLine,
    sound: &StreamSound,
    picture: &PictureSink,
    commands: &CommandReceiver<Command>,
    outbox: &Outbox,
) {
    let mut watching = Watching {
        key,
        line,
        sound,
        picture,
        outbox,
        source: None,
        decoder: None,
        shown: None,
        fit: DEFAULT_FIT,
        failure: None,
    };
    let waker = commands.waker();
    let mut heard = Context::from_waker(&waker);
    loop {
        for command in commands.drain() {
            match command {
                Command::Fit(width, height) => watching.fit = (width.max(1), height.max(1)),
                Command::Stop => return,
            }
        }
        loop {
            match events.poll_next_unpin(&mut heard) {
                Poll::Ready(Some(VoiceEvent::Ended(error))) => {
                    outbox.send(StreamUpdate::Ended(error.map(|error| error.to_string())));
                    return;
                }
                Poll::Ready(Some(event)) => watching.handle_event(event),
                Poll::Ready(None) => {
                    outbox.send(StreamUpdate::Ended(Some("stream connection closed".into())));
                    return;
                }
                Poll::Pending => break,
            }
            if let Some(failure) = watching.failure.take() {
                outbox.send(StreamUpdate::Ended(Some(failure)));
                return;
            }
        }
        if let Err(error) = commands.wait() {
            outbox.send(StreamUpdate::Ended(Some(error.to_string())));
            return;
        }
    }
}

impl Watching<'_> {
    fn handle_event(&mut self, event: VoiceEvent) {
        match event {
            VoiceEvent::Viewers(viewers) => self.outbox.send(StreamUpdate::Viewers(viewers)),
            VoiceEvent::Paused(paused) => self.outbox.send(StreamUpdate::Paused(paused)),
            VoiceEvent::VideoSource { source, user } if user == self.key.owner => {
                self.source = Some(source);
            }
            VoiceEvent::VideoGone { source } if self.source == Some(source) => {
                self.source = None;
            }
            VoiceEvent::Video(frame) => self.show(frame),
            VoiceEvent::Packet(packet) => self.sound.packet(packet),
            _ => {}
        }
    }

    fn show(&mut self, frame: VideoFrame) {
        let fit = self.fit;
        if self.source.is_some_and(|source| source != frame.source) {
            return;
        }
        let Some(decoder) = self.decoder_for(frame.codec) else {
            return;
        };
        match decoder.decode(&frame.data, Some(fit)) {
            Ok(pictures) => {
                if let Some(newest) = pictures.into_iter().last() {
                    let size = (newest.width, newest.height);
                    (self.picture)(newest.width, newest.height, newest.rgba);
                    if self.shown != Some(size) {
                        self.shown = Some(size);
                        self.outbox.send(StreamUpdate::Showing {
                            width: size.0,
                            height: size.1,
                        });
                    }
                }
            }
            Err(error) => {
                tracing::debug!(%error, "failed to decode watched frame");
                self.line.refresh(frame.source);
            }
        }
    }

    fn decoder_for(&mut self, codec: VideoCodec) -> Option<&mut LiveDecoder> {
        if self
            .decoder
            .as_ref()
            .is_none_or(|(known, _)| *known != codec)
        {
            self.decoder = match LiveDecoder::open(live(codec)) {
                Ok(decoder) => Some((codec, decoder)),
                Err(error) => {
                    tracing::warn!(%error, ?codec, "no decoder for stream codec");
                    self.failure = Some(error.to_string());
                    None
                }
            };
        }
        self.decoder.as_mut().map(|(_, decoder)| decoder)
    }
}

fn live(codec: VideoCodec) -> LiveCodec {
    match codec {
        VideoCodec::H264 => LiveCodec::H264,
        VideoCodec::H265 => LiveCodec::Hevc,
        VideoCodec::Vp8 => LiveCodec::Vp8,
        VideoCodec::Vp9 => LiveCodec::Vp9,
        VideoCodec::Av1 => LiveCodec::Av1,
    }
}
