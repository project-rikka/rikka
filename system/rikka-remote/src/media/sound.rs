use std::ops::ControlFlow;

use rikka_audio::{AudioCapture, CaptureConfig, CaptureSource, Sent, SoundEncoding, SoundSender};
use rikka_opus::{OPUS_SAMPLE_RATE, OpusApplication};
use rikka_remote_wire::AUDIO_FRAME_SAMPLES;

use crate::media::{MediaError, MediaEvent, MediaSink, SoundPacket};

const THREAD: &str = "rikka-remote-sound";
const CHANNELS: u16 = 2;

#[derive(Debug)]
pub(crate) struct SoundWorker {
    _sender: SoundSender,
}

impl SoundWorker {
    pub(crate) fn start(sink: MediaSink) -> Result<SoundWorker, MediaError> {
        let capture = AudioCapture::start(
            CaptureConfig::default()
                .source(CaptureSource::SystemAudio)
                .sample_rate(OPUS_SAMPLE_RATE)
                .channels(CHANNELS),
        )
        .map_err(MediaError::Sound)?;
        let encoding = SoundEncoding::new(
            THREAD,
            OpusApplication::LowDelay,
            AUDIO_FRAME_SAMPLES as usize,
        );
        let sender = SoundSender::start(capture, encoding, move |sent| match sent {
            Sent::Packet { data, at } => {
                let packet = SoundPacket {
                    data: data.to_vec(),
                    samples: at,
                };
                match sink.send(MediaEvent::Sound(packet)) {
                    Ok(()) => ControlFlow::Continue(()),
                    Err(_) => ControlFlow::Break(()),
                }
            }
            Sent::Quiet => ControlFlow::Continue(()),
        })
        .map_err(MediaError::Sound)?;
        Ok(SoundWorker { _sender: sender })
    }
}
