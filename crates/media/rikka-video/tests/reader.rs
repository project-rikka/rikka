use std::path::Path;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use rikka_video::{EncodeSpeed, EncoderBackend, EncoderConfig, FrameReader, VideoEncoder};

const WIDTH: u32 = 64;
const HEIGHT: u32 = 48;
const FPS: u32 = 10;
const FRAMES: u32 = 40;
const PATIENCE: Duration = Duration::from_secs(30);

#[derive(Clone, Debug)]
struct Shot {
    middle: [u8; 4],
}

fn level(index: u32) -> u8 {
    (16 + index * 5) as u8
}

fn index_of(level: u8) -> u32 {
    ((f64::from(level) - 16.0) / 5.0).round() as u32
}

fn at(index: u32) -> Duration {
    Duration::from_millis(u64::from(index) * 1000 / u64::from(FPS))
}

fn clip(path: &Path, pixel: impl Fn(u32) -> [u8; 4]) -> bool {
    let config = EncoderConfig::new(WIDTH, HEIGHT)
        .frame_rate(FPS)
        .backend(EncoderBackend::Software)
        .speed(EncodeSpeed::Balanced)
        .quality(100);
    let Ok(mut encoder) = VideoEncoder::new(None, path, config)
        .inspect_err(|error| eprintln!("skipping: nothing can encode here ({error})"))
    else {
        return false;
    };
    for index in 0..FRAMES {
        encoder
            .push_frame(&pixel(index).repeat((WIDTH * HEIGHT) as usize))
            .expect("push frame");
    }
    encoder.finish().expect("finish");
    true
}

fn grey(index: u32) -> [u8; 4] {
    let level = level(index);
    [level, level, level, 255]
}

struct Reading {
    reader: FrameReader<Shot>,
    ready: Receiver<()>,
}

impl Reading {
    fn open(path: &Path) -> Self {
        let reader = FrameReader::open(path, |frame| {
            let middle = ((frame.height / 2 * frame.width + frame.width / 2) * 4) as usize;
            let mut pixel = [0; 4];
            pixel.copy_from_slice(&frame.rgba[middle..middle + 4]);
            assert_eq!(frame.rgba.len(), (frame.width * frame.height * 4) as usize);
            Some(Shot { middle: pixel })
        })
        .expect("a reader");
        let (wake, ready) = channel();
        reader.on_ready(move || {
            let _ = wake.send(());
        });
        Self { reader, ready }
    }

    fn shown(&self, time: Duration, ahead: Duration) -> (Duration, Shot) {
        self.reader.want(time, ahead);
        self.awaited(time)
    }

    fn awaited(&self, time: Duration) -> (Duration, Shot) {
        let deadline = Instant::now() + PATIENCE;
        loop {
            if let Some(found) = self.reader.frame(time) {
                return found;
            }
            assert_eq!(self.reader.failure(), None, "the reader failed");
            let left = deadline
                .checked_duration_since(Instant::now())
                .unwrap_or_else(|| panic!("no frame for {time:?} in time"));
            let _ = self.ready.recv_timeout(left);
        }
    }
}

fn assert_frame(found: &(Duration, Shot), index: u32, time: Duration) {
    let (pts, shot) = found;
    assert_eq!(
        *pts,
        at(index),
        "the frame shown at {time:?} starts at {:?}",
        at(index)
    );
    assert_eq!(
        index_of(shot.middle[0]),
        index,
        "the frame shown at {time:?} is frame {index}: {shot:?}"
    );
}

fn greys(container: &str) -> Option<(tempfile::TempDir, std::path::PathBuf)> {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join(format!("greys.{container}"));
    clip(&path, grey).then_some((directory, path))
}

#[test]
fn frame_at_returns_covering_frame() {
    for container in ["mkv", "mp4"] {
        let Some((_directory, path)) = greys(container) else {
            return;
        };
        let reading = Reading::open(&path);

        for time in [2.55, 0.0, 3.85, 1.0, 1.99, 0.05, 3.0, 2.0, 0.42, 1.95, 2.01] {
            let time = Duration::from_secs_f64(time);
            let index = (time.as_secs_f64() * f64::from(FPS) + 1e-6).floor() as u32;
            assert_frame(&reading.shown(time, Duration::ZERO), index, time);
        }

        let info = reading.reader.info().expect("probed once a frame is shown");
        assert_eq!((info.width, info.height), (WIDTH, HEIGHT));
    }
}

#[test]
fn seek_during_decode_lands_on_frame() {
    let Some((_directory, path)) = greys("mkv") else {
        return;
    };
    let reading = Reading::open(&path);

    reading.reader.want(Duration::ZERO, Duration::from_secs(3));
    reading.reader.want(at(33), Duration::ZERO);
    assert_frame(&reading.awaited(at(33)), 33, at(33));
    reading.reader.want(at(7), Duration::ZERO);
    assert_frame(&reading.awaited(at(7)), 7, at(7));
}

#[test]
fn colour_matrix_round_trips() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("colours.mp4");
    let colours = [[200, 40, 40, 255], [40, 180, 60, 255], [50, 70, 210, 255]];
    if !clip(&path, |index| colours[index as usize % colours.len()]) {
        return;
    }
    let reading = Reading::open(&path);

    for (index, colour) in colours.iter().enumerate() {
        let (_, shot) = reading.shown(at(index as u32 + 6), Duration::ZERO);
        for channel in 0..3 {
            assert!(
                shot.middle[channel].abs_diff(colour[channel]) <= 8,
                "{colour:?} came back as {:?}",
                shot.middle
            );
        }
    }
}

#[test]
fn missing_file_reports_failure() {
    let directory = tempfile::tempdir().unwrap();
    let Ok(reader) = FrameReader::open(directory.path().join("missing.mp4"), |_| Some(())) else {
        return;
    };
    let (wake, woken) = channel();
    reader.on_ready(move || {
        let _ = wake.send(());
    });
    let deadline = Instant::now() + PATIENCE;
    while reader.failure().is_none() {
        let Some(left) = deadline.checked_duration_since(Instant::now()) else {
            break;
        };
        if woken.recv_timeout(left).is_err() {
            break;
        }
    }
    assert!(reader.failure().is_some(), "no failure was reported");
    assert!(reader.info().is_none());
    assert!(reader.frame(Duration::ZERO).is_none());
}
