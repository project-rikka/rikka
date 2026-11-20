use std::path::Path;

use discord_krisp::{Error, Module};

fn touch(folder: &Path, names: &[&str]) {
    for name in names {
        std::fs::write(folder.join(name), b"").unwrap();
    }
}

#[test]
fn module_selects_fullband_nc_and_newest_vad() {
    let folder = tempfile::tempdir().unwrap();
    touch(
        folder.path(),
        &[
            "discord_krisp.node",
            "krisp-nc-o-nb-v9.kef",
            "krisp-nc-o-lite-v8.kef",
            "krisp-nc-o-med-v7.kef",
            "krisp-nc-o-med-v12.kef",
            "krisp-bvc-o-pro-v3.kef",
            "krisp-vad-o-v2.kef",
            "krisp-vad-o-v10.kef",
        ],
    );

    let module = Module::from_folder(folder.path()).unwrap();

    assert_eq!(
        module.noise_model(),
        folder.path().join("krisp-nc-o-med-v12.kef")
    );
    assert_eq!(
        module.voice_model(),
        folder.path().join("krisp-vad-o-v10.kef")
    );
}

#[test]
fn incomplete_module_is_rejected() {
    let cases: [(&[&str], &str); 3] = [
        (
            &["krisp-nc-o-med-v7.kef", "krisp-vad-o-v2.kef"],
            "discord_krisp.node",
        ),
        (
            &[
                "discord_krisp.node",
                "krisp-nc-o-nb-v2.kef",
                "krisp-vad-o-v2.kef",
            ],
            "a noise model",
        ),
        (
            &["discord_krisp.node", "krisp-nc-o-med-v7.kef"],
            "a voice activity model",
        ),
    ];
    for (present, expected_missing) in cases {
        let folder = tempfile::tempdir().unwrap();
        touch(folder.path(), present);
        match Module::from_folder(folder.path()) {
            Err(Error::Incomplete { missing, .. }) => assert_eq!(missing, expected_missing),
            other => panic!("{present:?} gave {other:?}"),
        }
    }
}

#[test]
fn cached_serves_newest_complete_release() {
    let cache = tempfile::tempdir().unwrap();
    for release in ["1.0.9-1", "1.0.12-1", "1.0.20-1"] {
        let folder = cache.path().join(release);
        std::fs::create_dir(&folder).unwrap();
        if release == "1.0.20-1" {
            continue;
        }
        touch(
            &folder,
            &[
                "discord_krisp.node",
                "krisp-nc-o-med-v7.kef",
                "krisp-vad-o-v2.kef",
            ],
        );
    }

    let module = Module::cached(cache.path()).unwrap();

    assert_eq!(module.folder(), cache.path().join("1.0.12-1"));
}

#[test]
fn prune_keeps_loaded_release_skips_symlinks() {
    let cache = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let parts = [
        "discord_krisp.node",
        "krisp-nc-o-med-v7.kef",
        "krisp-vad-o-v2.kef",
    ];
    touch(elsewhere.path(), &parts);
    for release in ["1.0.9-1", "1.0.12-1"] {
        let folder = cache.path().join(release);
        std::fs::create_dir(&folder).unwrap();
        touch(&folder, &parts);
    }
    std::os::unix::fs::symlink(elsewhere.path(), cache.path().join("1.0.1-1")).unwrap();
    std::fs::create_dir(cache.path().join("work")).unwrap();
    std::fs::write(cache.path().join(".1.0.9-1.distro.part"), b"").unwrap();
    std::fs::create_dir(cache.path().join(".1.0.9-1.unpacking")).unwrap();

    let loaded = Module::cached(cache.path()).unwrap();
    loaded.prune_others(cache.path());

    assert!(cache.path().join("1.0.12-1").is_dir());
    assert!(!cache.path().join("1.0.9-1").exists());
    assert!(!cache.path().join(".1.0.9-1.distro.part").exists());
    assert!(!cache.path().join(".1.0.9-1.unpacking").exists());
    assert!(cache.path().join("work").is_dir());
    assert!(elsewhere.path().join("discord_krisp.node").is_file());
}
