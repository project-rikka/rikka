use rikka_sound::SampleFormat;
use rikka_sound::config::{DeviceTuning, DeviceTunings, GraphTuning};

#[test]
fn pins_round_trip_and_empty_removes_file() {
    let home = tempfile::tempdir().expect("temp dir should be created");
    unsafe { std::env::set_var("XDG_CONFIG_HOME", home.path()) };

    let graph = GraphTuning {
        rate: Some(96000),
        allowed_rates: vec![44100, 48000, 96000],
        quantum: Some(1024),
        min_quantum: None,
        max_quantum: Some(8192),
        resample_quality: Some(10),
    };
    graph.write().expect("graph should be pinned");
    assert_eq!(GraphTuning::read(), graph);

    let mut devices = DeviceTunings::default();
    devices.set(
        "alsa_output.pci-0000_00_1f.3.analog-stereo",
        DeviceTuning {
            format: Some(SampleFormat::S32),
            rate: Some(96000),
            allowed_rates: vec![44100, 48000, 96000],
            period: Some(1024),
            headroom: Some(0),
            disable_batch: true,
        },
    );
    devices.set(
        "alsa_output.usb-dac",
        DeviceTuning {
            format: Some(SampleFormat::S24In32),
            ..DeviceTuning::default()
        },
    );
    devices.write().expect("devices should be pinned");
    assert_eq!(DeviceTunings::read(), devices);

    devices.set("alsa_output.usb-dac", DeviceTuning::default());
    assert_eq!(devices.get("alsa_output.usb-dac"), None);
    assert!(
        devices
            .get("alsa_output.pci-0000_00_1f.3.analog-stereo")
            .is_some()
    );

    GraphTuning::default()
        .write()
        .expect("graph should be released");
    DeviceTunings::default()
        .write()
        .expect("devices should be released");
    assert_eq!(GraphTuning::read(), GraphTuning::default());
    assert_eq!(DeviceTunings::read(), DeviceTunings::default());
    let left: Vec<_> = walk(home.path());
    assert!(left.is_empty(), "nothing pinned leaves no file: {left:?}");
}

fn walk(folder: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(folder)
        .expect("directory should be readable")
        .flatten()
    {
        let path = entry.path();
        if path.is_dir() {
            found.extend(walk(&path));
        } else {
            found.push(path);
        }
    }
    found
}
