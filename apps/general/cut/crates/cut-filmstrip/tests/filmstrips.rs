use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

use cut_filmstrip::{Filmstrips, Request, decoded_height};
use rikka_video::{EncodeSpeed, EncoderBackend, EncoderConfig, VideoEncoder};

const WIDTH: u32 = 64;
const HEIGHT: u32 = 36;
const FPS: u32 = 10;
const TIMEOUT: Duration = Duration::from_secs(30);

fn greys(folder: &Path) -> Option<PathBuf> {
    let path = folder.join("greys.mp4");
    let config = EncoderConfig::new(WIDTH, HEIGHT)
        .frame_rate(FPS)
        .backend(EncoderBackend::Software)
        .speed(EncodeSpeed::Balanced)
        .quality(100);
    let Ok(mut encoder) = VideoEncoder::new(None, &path, config)
        .inspect_err(|error| eprintln!("skipping: nothing can encode here ({error})"))
    else {
        return None;
    };
    for index in 0..30u32 {
        let value = (20 + index * 7) as u8;
        encoder
            .push_frame(&[value, value, value, 255].repeat((WIDTH * HEIGHT) as usize))
            .expect("push frame");
    }
    encoder.finish().expect("finish");
    Some(path)
}

fn request(path: &Path, frame: u64) -> Request {
    Request {
        path: Arc::from(path),
        at: Duration::from_millis(frame * 100),
        tolerance: Duration::ZERO,
        height: decoded_height(20.0),
    }
}

#[test]
fn requested_thumbnails_arrive_exact_and_sized() {
    let folder = tempfile::tempdir().expect("a folder");
    let Some(path) = greys(folder.path()) else {
        return;
    };
    let filmstrips = Filmstrips::new().expect("started decoders");
    let (notify, notified) = channel();
    filmstrips.on_change(move || {
        let _ = notify.send(());
    });
    let requests: Vec<Request> = [25, 3, 14].map(|frame| request(&path, frame)).to_vec();
    filmstrips.set_requests(requests.clone());

    let deadline = Instant::now() + TIMEOUT;
    for expected in &requests {
        let matched = loop {
            match filmstrips.find(expected) {
                Some(matched) if matched.exact => break matched,
                _ => {
                    let left = deadline
                        .checked_duration_since(Instant::now())
                        .expect("the thumbnails arrived in time");
                    let _ = notified.recv_timeout(left);
                }
            }
        };
        assert_eq!(matched.thumbnail.pts, expected.at);
        assert_eq!(matched.thumbnail.height, decoded_height(20.0).min(HEIGHT));
        assert_eq!(
            matched.thumbnail.pixels.len(),
            (matched.thumbnail.width * matched.thumbnail.height * 4) as usize
        );
        let level = matched.thumbnail.pixels[0];
        let index = (expected.at.as_millis() / 100) as i64;
        assert!(
            (i64::from(level) - (20 + index * 7)).abs() <= 4,
            "frame {index} shows {level}"
        );
    }

    let nearby = Request {
        at: Duration::from_millis(1450),
        tolerance: Duration::from_millis(100),
        ..request(&path, 0)
    };
    let stand_in = filmstrips
        .find(&nearby)
        .expect("a nearby thumbnail stands in");
    assert!(!stand_in.exact);
    assert_eq!(stand_in.thumbnail.pts, Duration::from_millis(1400));
}

#[test]
fn unreadable_footage_is_not_retried() {
    let folder = tempfile::tempdir().expect("a folder");
    let broken = folder.path().join("broken.mp4");
    std::fs::write(&broken, b"not a film").expect("writing");
    let filmstrips = Filmstrips::new().expect("started decoders");
    let (notify, notified) = channel();
    filmstrips.on_change(move || {
        let _ = notify.send(());
    });

    filmstrips.set_requests(vec![request(&broken, 0), request(&broken, 3)]);

    assert!(notified.recv_timeout(TIMEOUT).is_ok());
    let mut tries = 1;
    while notified.recv_timeout(Duration::from_millis(300)).is_ok() {
        tries += 1;
    }
    assert!(tries <= 2, "the footage was tried {tries} times");
    assert!(filmstrips.find(&request(&broken, 0)).is_none());
}

#[test]
fn forgotten_footage_is_read_again() {
    let folder = tempfile::tempdir().expect("a folder");
    let film = folder.path().join("film.mp4");
    std::fs::write(&film, b"not a film yet").expect("writing");
    let filmstrips = Filmstrips::new().expect("started decoders");
    let (notify, notified) = channel();
    filmstrips.on_change(move || {
        let _ = notify.send(());
    });
    filmstrips.set_requests(vec![request(&film, 3)]);
    notified
        .recv_timeout(TIMEOUT)
        .expect("the failure is reported");
    let Some(greys) = greys(folder.path()) else {
        return;
    };
    std::fs::rename(greys, &film).expect("the footage returns");

    filmstrips.forget(&film);

    let deadline = Instant::now() + TIMEOUT;
    while !filmstrips
        .find(&request(&film, 3))
        .is_some_and(|matched| matched.exact)
    {
        let left = deadline
            .checked_duration_since(Instant::now())
            .expect("the returned footage was read in time");
        let _ = notified.recv_timeout(left);
    }
}
