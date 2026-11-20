use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use cut_waveform::{Summary, Waveform, Waveforms};
use rikka_audio_decode::{WavDepth, WavWriter};

const RATE: u32 = 8_000;
const TIMEOUT: Duration = Duration::from_secs(30);

fn tone(path: &Path, amplitude: f32, seconds: u32) {
    let mut writer = WavWriter::create(path, RATE, 2, WavDepth::Pcm16).expect("a wav");
    let samples: Vec<f32> = (0..RATE * seconds)
        .flat_map(|index| {
            let wave = (index as f32 / RATE as f32 * 440.0 * std::f32::consts::TAU).sin();
            [wave * amplitude, -wave * amplitude * 0.5]
        })
        .collect();
    writer.write(&samples).expect("writing");
    writer.finish().expect("finishing");
}

fn watched(cache: Option<&Path>) -> (Waveforms, Receiver<()>) {
    let waveforms = Waveforms::new(cache.map(Path::to_path_buf)).expect("the decoders start");
    let (notify, notified) = channel();
    waveforms.on_change(move || {
        let _ = notify.send(());
    });
    (waveforms, notified)
}

fn whole(waveform: &Waveform, notified: &Receiver<()>) -> Summary {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        assert_eq!(waveform.failure(), None);
        let read = waveform.with_peaks(|peaks| {
            peaks
                .is_complete()
                .then(|| peaks.summary(0, 0..peaks.frames()))
                .flatten()
        });
        if let Some(Some(summary)) = read {
            return summary;
        }
        let left = deadline
            .checked_duration_since(Instant::now())
            .expect("the waveform was read in time");
        let _ = notified.recv_timeout(left);
    }
}

type CacheFiles = BTreeMap<PathBuf, Vec<u8>>;

fn cache_files(cache: &Path) -> CacheFiles {
    std::fs::read_dir(cache)
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|entry| Some((entry.path(), std::fs::read(entry.path()).ok()?)))
                .collect()
        })
        .unwrap_or_default()
}

fn rewritten(cache: &Path, notified: &Receiver<()>, before: &CacheFiles) -> CacheFiles {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let now = cache_files(cache);
        if !now.is_empty() && now != *before {
            return now;
        }
        let left = deadline
            .checked_duration_since(Instant::now())
            .expect("the waveform was kept in time");
        let _ = notified.recv_timeout(left);
    }
}

#[test]
fn second_read_comes_from_cache() {
    let folder = tempfile::tempdir().expect("a folder");
    let cache = folder.path().join("cache");
    let sound = folder.path().join("tone.wav");
    tone(&sound, 0.5, 3);

    let (waveforms, notified) = watched(Some(&cache));
    let read = whole(&waveforms.waveform(&sound), &notified);
    assert!((read.high - 0.5).abs() < 0.01 && (read.low + 0.5).abs() < 0.01);
    assert!((read.rms - 0.5 / 2f32.sqrt()).abs() < 0.01);
    assert_eq!(rewritten(&cache, &notified, &CacheFiles::new()).len(), 1);
    drop(waveforms);

    let stamped = std::fs::metadata(&sound).expect("the sound");
    let silence = vec![0u8; stamped.len() as usize];
    std::fs::write(&sound, silence).expect("silencing the sound");
    std::fs::File::options()
        .write(true)
        .open(&sound)
        .and_then(|file| file.set_modified(stamped.modified()?))
        .expect("keeping the time");
    let (reopened, notified) = watched(Some(&cache));
    assert_eq!(whole(&reopened.waveform(&sound), &notified), read);
}

#[test]
fn changed_or_corrupt_cache_is_reread() {
    let folder = tempfile::tempdir().expect("a folder");
    let cache = folder.path().join("cache");
    let sound = folder.path().join("tone.wav");
    tone(&sound, 0.5, 2);
    let (waveforms, notified) = watched(Some(&cache));
    whole(&waveforms.waveform(&sound), &notified);
    let first = rewritten(&cache, &notified, &CacheFiles::new());
    drop(waveforms);

    tone(&sound, 0.25, 3);
    let (changed, notified) = watched(Some(&cache));
    let read = whole(&changed.waveform(&sound), &notified);
    assert!((read.high - 0.25).abs() < 0.01);
    let second = rewritten(&cache, &notified, &first);
    drop(changed);

    for file in second.into_keys() {
        std::fs::write(file, b"RCWF garbage").expect("damaging the cache");
    }
    let (damaged, notified) = watched(Some(&cache));
    let read = whole(&damaged.waveform(&sound), &notified);
    assert!((read.high - 0.25).abs() < 0.01);
}

#[test]
fn undecodable_file_reports_failure() {
    let folder = tempfile::tempdir().expect("a folder");
    let noise = folder.path().join("noise.wav");
    std::fs::write(&noise, b"not a sound at all").expect("writing");
    let (waveforms, notified) = watched(None);
    let waveform = waveforms.waveform(&noise);

    notified.recv_timeout(TIMEOUT).expect("failure is reported");
    assert!(waveform.failure().is_some());
    assert!(waveform.with_peaks(|_| ()).is_none());
}

#[test]
fn forgotten_file_is_read_again() {
    let folder = tempfile::tempdir().expect("a folder");
    let sound = folder.path().join("sound.wav");
    std::fs::write(&sound, b"not a sound yet").expect("writing");
    let (waveforms, notified) = watched(None);
    let failed = waveforms.waveform(&sound);
    notified.recv_timeout(TIMEOUT).expect("failure is reported");
    assert!(failed.failure().is_some());
    tone(&sound, 0.5, 1);

    waveforms.forget(&sound);

    let read = whole(&waveforms.waveform(&sound), &notified);
    assert!((read.high - 0.5).abs() < 0.01);
}
