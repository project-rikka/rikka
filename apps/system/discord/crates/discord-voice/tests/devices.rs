use std::time::Duration;

use discord_voice::{
    Error, Krisp, KrispState, MicTest, NoiseSuppression, VoiceSettings, input_devices,
    output_devices,
};
use futures_util::StreamExt;

const KRISP_PATIENCE: Duration = Duration::from_secs(10);
const LEVEL_WINDOW: Duration = Duration::from_secs(1);

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap()
}

#[test]
fn lists_input_and_output_devices() {
    let inputs = input_devices().unwrap();
    let outputs = output_devices().unwrap();

    assert!(!outputs.is_empty(), "no output is listed");
    assert!(inputs.iter().filter(|device| device.is_default).count() <= 1);
    assert!(outputs.iter().filter(|device| device.is_default).count() <= 1);
}

fn ready_krisp(cache: &std::path::Path) -> Krisp {
    for release in std::fs::read_dir(env!("RIKKA_DISCORD_KRISP")).unwrap() {
        let release = release.unwrap();
        std::os::unix::fs::symlink(release.path(), cache.join(release.file_name())).unwrap();
    }
    let krisp = Krisp::new(cache);
    krisp.prepare().unwrap();
    let mut states = krisp.watch();
    let loaded = runtime().block_on(async {
        tokio::time::timeout(KRISP_PATIENCE, async {
            while let Some(state) = states.next().await {
                match state {
                    KrispState::Ready => return Ok(()),
                    KrispState::Unavailable(reason) => return Err(reason),
                    _ => {}
                }
            }
            Err("Krisp state stream closed".to_owned())
        })
        .await
    });
    match loaded {
        Ok(Ok(())) => krisp,
        Ok(Err(reason)) => panic!("Krisp unavailable: {reason}"),
        Err(_) => panic!("Krisp not ready within {KRISP_PATIENCE:?}"),
    }
}

#[test]
fn krisp_mic_test_reports_levels_at_20_hz() {
    let cache = tempfile::tempdir().unwrap();
    let krisp = ready_krisp(cache.path());
    let settings = VoiceSettings {
        noise_suppression: NoiseSuppression::Krisp,
        ..VoiceSettings::default()
    };
    let mut test = match MicTest::start(&settings, &krisp) {
        Ok(test) => test,
        Err(Error::Device(error)) => {
            eprintln!("the sound server is out of reach ({error}); skipping the mic test");
            return;
        }
        Err(Error::DeviceMissing(device)) => {
            eprintln!("no audio device named {device}; skipping the mic test");
            return;
        }
        Err(error) => panic!("the mic test does not start: {error}"),
    };
    test.set_loopback(true);
    let mut levels = test.levels();

    let mut received = Vec::new();
    let collecting = runtime().block_on(async {
        tokio::time::timeout(LEVEL_WINDOW, async {
            while let Some(level) = levels.next().await {
                received.push(level);
            }
        })
        .await
    });

    assert!(collecting.is_err(), "the level stream ended early");
    assert!(
        (10..=40).contains(&received.len()),
        "{} levels in a second",
        received.len()
    );
    assert!(received.iter().all(|level| (-100.0..=0.0).contains(level)));
}
