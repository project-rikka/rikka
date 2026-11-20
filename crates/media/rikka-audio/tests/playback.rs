use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use rikka_audio::{AudioEngine, EngineConfig, Error, PlayParams, PlaybackEvent};
use rikka_audio_decode::{Sound, WavDepth, WavWriter};
use rustix::fs::{Mode, OFlags};

struct Audited;

unsafe impl GlobalAlloc for Audited {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if is_audio_thread() {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if is_audio_thread() {
            FREES.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Audited = Audited;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static FREES: AtomicUsize = AtomicUsize::new(0);

const DATA_LOOP: &[u8] = b"data-loop";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Unknown,
    Audio,
    Other,
}

thread_local! {
    static ROLE: Cell<Role> = const { Cell::new(Role::Unknown) };
}

fn is_audio_thread() -> bool {
    ROLE.try_with(|role| {
        if role.get() == Role::Unknown {
            role.set(match thread_name().starts_with(DATA_LOOP) {
                true => Role::Audio,
                false => Role::Other,
            });
        }
        role.get() == Role::Audio
    })
    .unwrap_or(false)
}

fn thread_name() -> [u8; 16] {
    let mut name = [0; 16];
    if let Ok(comm) = rustix::fs::open(c"/proc/thread-self/comm", OFlags::RDONLY, Mode::empty()) {
        let _ = rustix::io::read(&comm, &mut name);
    }
    name
}

fn data_loop_runs() -> bool {
    std::fs::read_dir("/proc/self/task")
        .into_iter()
        .flatten()
        .flatten()
        .any(|task| {
            std::fs::read(task.path().join("comm")).is_ok_and(|comm| comm.starts_with(DATA_LOOP))
        })
}

const STARTUP: Duration = Duration::from_millis(1_500);
const ONE_SHOT_SPAN: Duration = Duration::from_millis(400);
const PLAY_SPAN: Duration = Duration::from_millis(300);
const PAUSE_SPAN: Duration = Duration::from_millis(100);
const STOP_SPAN: Duration = Duration::from_millis(200);
const PAUSED_STREAM_SPAN: Duration = Duration::from_millis(1_000);
const RESUMED_STREAM_SPAN: Duration = Duration::from_millis(1_500);
const BEYOND_SUSPEND: Duration = Duration::from_millis(3_000);
const AUDIBLE_SPAN: Duration = Duration::from_millis(500);

fn tone_samples(rate: u32, frames: usize) -> Vec<f32> {
    (0..frames)
        .flat_map(|frame| {
            [440.0f32, 880.0]
                .map(|hz| (std::f32::consts::TAU * hz * frame as f32 / rate as f32).sin() * 0.9)
        })
        .collect()
}

fn tone(rate: u32, frames: usize) -> Sound {
    Sound::from_samples(tone_samples(rate, frames), 2, rate)
}

fn write_tone(path: &std::path::Path, rate: u32, frames: usize) {
    let mut wav = WavWriter::create(path, rate, 2, WavDepth::Pcm16).unwrap();
    wav.write(&tone_samples(rate, frames)).unwrap();
    wav.finish().unwrap();
}

fn open_engine() -> Option<AudioEngine> {
    match AudioEngine::new(EngineConfig::default()) {
        Ok(engine) => Some(engine),
        Err(Error::NoOutputDevice) => {
            eprintln!("no audio output device; skipping engine test");
            None
        }
        Err(Error::PipeWire(err)) => {
            eprintln!("sound server unavailable ({err}); skipping engine test");
            None
        }
        Err(err) => panic!("unexpected engine error: {err}"),
    }
}

#[test]
fn engine_plays_overlapping_one_shots() {
    let Some(engine) = open_engine() else { return };
    let sound = tone(44100, 8820);

    let mut handles = Vec::new();
    for i in 0..16 {
        let params = PlayParams::new().volume(0.05).pan((i as f32 / 8.0) - 1.0);
        handles.push(engine.play_with(&sound, params).unwrap());
    }
    std::thread::sleep(STARTUP + ONE_SHOT_SPAN);
    for handle in &handles {
        assert!(
            handle.is_finished(),
            "a 200 ms one-shot should have finished"
        );
    }
}

#[test]
fn engine_looping_and_control() {
    let Some(engine) = open_engine() else { return };
    let sound = tone(44100, 4410);

    let handle = engine
        .play_with(&sound, PlayParams::new().volume(0.05).looping(true))
        .unwrap();
    std::thread::sleep(PLAY_SPAN);
    assert!(!handle.is_finished());

    handle.pause();
    std::thread::sleep(PAUSE_SPAN);
    let paused_at = handle.position();
    std::thread::sleep(PAUSE_SPAN);
    assert_eq!(
        paused_at,
        handle.position(),
        "position must freeze while paused"
    );

    handle.resume();
    handle.set_speed(1.5);
    handle.set_volume(0.02);
    handle.stop();
    std::thread::sleep(STARTUP + STOP_SPAN);
    assert!(handle.is_finished());
}

#[test]
fn engine_streams_file_with_seek() {
    let Some(engine) = open_engine() else { return };

    let dir = std::env::temp_dir();
    let path = dir.join("rikka_audio_stream_test.wav");
    write_tone(&path, 44100, 132300);

    let handle = engine
        .stream_with(&path, PlayParams::new().volume(0.05))
        .unwrap();
    let duration = handle.duration().expect("wav reports a duration");
    assert!((duration.as_secs_f64() - 3.0).abs() < 0.01);

    std::thread::sleep(STARTUP + PLAY_SPAN);
    assert!(handle.position() > Duration::ZERO);

    handle.seek(Duration::from_secs(2));
    std::thread::sleep(PLAY_SPAN);
    let position = handle.position();
    assert!(
        position >= Duration::from_millis(1900) && position < Duration::from_millis(2700),
        "expected ~2 s after seek, got {position:?}",
    );

    handle.stop();
    std::thread::sleep(PAUSE_SPAN);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn watched_stream_reports_progress_then_its_end_once() {
    let Some(engine) = open_engine() else { return };
    let path = std::env::temp_dir().join("rikka_audio_watch_test.wav");
    write_tone(&path, 44100, 22050);

    let handle = engine
        .stream_with(&path, PlayParams::new().volume(0.05))
        .unwrap();
    let (heard, events) = std::sync::mpsc::channel();
    let _watch = handle.watch(move |event| {
        let _ = heard.send(event);
    });

    let mut progressed = false;
    loop {
        match events.recv_timeout(STARTUP + BEYOND_SUSPEND) {
            Ok(PlaybackEvent::Progressed) => progressed = true,
            Ok(PlaybackEvent::Finished) => break,
            Err(error) => panic!("the stream never reported its end: {error}"),
        }
    }
    assert!(
        progressed,
        "a playing stream reports progress before it ends"
    );
    assert!(handle.is_finished());
    assert!(
        events.recv_timeout(PAUSE_SPAN).is_err(),
        "the end is told once"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn long_paused_stream_resumes() {
    let Some(engine) = open_engine() else { return };
    let path = std::env::temp_dir().join("rikka_audio_paused_stream_test.wav");
    write_tone(&path, 44100, 220500);

    let handle = engine
        .stream_with(&path, PlayParams::new().volume(0.02))
        .unwrap();
    std::thread::sleep(STARTUP);
    handle.pause();
    std::thread::sleep(PAUSED_STREAM_SPAN);
    let paused_at = handle.position();
    handle.resume();
    std::thread::sleep(RESUMED_STREAM_SPAN);
    let played = handle.position().saturating_sub(paused_at);
    assert!(
        played >= Duration::from_millis(1_000),
        "a resumed stream must keep decoding past what it had buffered, played {played:?}"
    );

    handle.stop();
    std::thread::sleep(PAUSE_SPAN);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn suspended_output_resumes_paused_voices() {
    let Some(engine) = open_engine() else { return };
    let sound = tone(44100, 44100);
    let paused = engine
        .play_with(&sound, PlayParams::new().volume(0.02).looping(true))
        .unwrap();
    std::thread::sleep(STARTUP);
    paused.pause();
    std::thread::sleep(BEYOND_SUSPEND);
    let paused_at = paused.position();
    paused.resume();
    std::thread::sleep(AUDIBLE_SPAN);
    assert!(
        paused.position() != paused_at,
        "a paused voice must play again once resumed after the output rested"
    );
    paused.stop();

    std::thread::sleep(BEYOND_SUSPEND);
    let fresh = engine
        .play_with(&sound, PlayParams::new().volume(0.02))
        .unwrap();
    std::thread::sleep(AUDIBLE_SPAN);
    assert!(
        fresh.position() > Duration::ZERO,
        "a sound played after the output rested must be heard"
    );
    fresh.stop();
}

#[test]
fn feed_wait_returns_once_output_drains() {
    let Some(engine) = open_engine() else { return };
    let (_handle, mut feed) = engine.feed(48_000, 2).unwrap();
    feed.write(&vec![0.0; 48_000 / 10 * 2]).unwrap();
    let queued = feed.queued();
    assert!(feed.wait_until_queued_below(queued, STARTUP));
    assert!(feed.queued() < queued);
}

#[test]
fn audio_thread_never_allocates() {
    let Some(engine) = open_engine() else { return };
    let sound = tone(48000, 960);
    std::thread::sleep(STARTUP);
    assert!(
        data_loop_runs(),
        "the engine renders on PipeWire's data loop"
    );
    ALLOCATIONS.store(0, Ordering::Relaxed);
    FREES.store(0, Ordering::Relaxed);

    let handles: Vec<_> = (0..32)
        .map(|_| {
            engine
                .play_with(&sound, PlayParams::new().volume(0.01))
                .unwrap()
        })
        .collect();
    std::thread::sleep(AUDIBLE_SPAN);
    assert!(handles.iter().all(|handle| handle.is_finished()));

    assert_eq!(
        ALLOCATIONS.load(Ordering::Relaxed),
        0,
        "the audio callback allocated"
    );
    assert_eq!(
        FREES.load(Ordering::Relaxed),
        0,
        "the audio callback freed memory"
    );
}
