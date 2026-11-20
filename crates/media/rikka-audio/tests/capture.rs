use std::sync::mpsc;
use std::time::{Duration, Instant};

use rikka_audio::{
    AudioCapture, AudioEngine, CaptureConfig, CaptureSource, DeviceEvent, DeviceWatch,
    EngineConfig, Fill,
};

const CAPTURE_TIMEOUT: Duration = Duration::from_secs(2);
const BLOCK_DURATION: Duration = Duration::from_millis(20);
const TONE_SECONDS: usize = 3;
const TONE_RATE: u32 = 48_000;
const TONE_HZ: f32 = 440.0;

#[test]
fn fill_takes_a_block_from_the_playing_output() {
    let engine = AudioEngine::new(EngineConfig::default()).expect("default output opens");
    let (_handle, mut feed) = engine.feed(TONE_RATE, 1).expect("feed opens");
    let tone: Vec<f32> = (0..TONE_RATE as usize * TONE_SECONDS)
        .map(|frame| {
            (frame as f32 * TONE_HZ * std::f32::consts::TAU / TONE_RATE as f32).sin() * 0.05
        })
        .collect();
    feed.write(&tone).expect("tone queues");
    let capture = AudioCapture::start(CaptureConfig::default().source(CaptureSource::SystemAudio))
        .expect("output monitor opens");
    let frames = capture.sample_rate() as usize * BLOCK_DURATION.as_millis() as usize / 1000;
    let mut block = vec![0.0; frames * usize::from(capture.channels())];
    let started = Instant::now();
    assert_eq!(capture.fill(&mut block, CAPTURE_TIMEOUT), Fill::Filled);
    assert!(started.elapsed() < CAPTURE_TIMEOUT);
}

#[test]
fn device_watch_reports_present_devices() {
    let (sender, receiver) = mpsc::channel();
    let _watch = DeviceWatch::start(move |event| {
        sender.send(event).ok();
    })
    .expect("device watch starts");
    let event = receiver
        .recv_timeout(CAPTURE_TIMEOUT)
        .expect("PipeWire announces at least one audio device");
    assert!(matches!(event, DeviceEvent::Added { .. }));
}
