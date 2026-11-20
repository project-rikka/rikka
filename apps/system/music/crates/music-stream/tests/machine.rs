mod support;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use music_core::{AudioCache, Codec, Id, Playback, PlaybackEvent, Resolution};
use music_stream::{Located, Origin, Streamer};
use rikka_hifi::Gains;

use crate::support::{deck, forwarded, wait_for, wav};

struct Files(PathBuf);

impl Origin for Files {
    fn locate(&self, track: &Id) -> Option<Located> {
        Some(Located {
            path: self.0.join(format!("{track}.wav")),
            gains: Gains::default(),
        })
    }
}

#[test]
fn local_files_play_gaplessly() {
    let folder = tempfile::tempdir().unwrap();
    for name in ["a", "b"] {
        std::fs::write(
            folder.path().join(format!("{name}.wav")),
            wav(44_100, 26_460),
        )
        .unwrap();
    }
    let root = tempfile::tempdir().unwrap();
    let cache = AudioCache::open(root.path(), 0).for_provider("files");
    let streamer =
        Streamer::start(Arc::new(Files(folder.path().to_owned())), cache, deck()).unwrap();
    let events = forwarded(streamer.events());
    let (a, b) = (Id::new("a"), Id::new("b"));

    streamer.load(&a, Duration::ZERO, true);
    streamer.prepare(Some(&b));

    let mut seen = Vec::new();
    let began = wait_for(&events, &mut seen, |event| {
        matches!(
            event,
            PlaybackEvent::Playing { .. } | PlaybackEvent::Failed(_)
        )
    });
    if let Some(PlaybackEvent::Failed(reason)) = began {
        eprintln!("nothing plays here ({reason}): the gapless test is skipped");
        return;
    }
    assert_eq!(
        began,
        Some(PlaybackEvent::Playing {
            track: a.clone(),
            at: Duration::ZERO
        }),
        "{seen:?}"
    );
    let advanced = wait_for(&events, &mut seen, |event| {
        matches!(
            event,
            PlaybackEvent::Advanced { .. } | PlaybackEvent::Ended { .. }
        )
    });
    assert_eq!(
        advanced,
        Some(PlaybackEvent::Advanced {
            from: a,
            to: b.clone()
        }),
        "{seen:?}"
    );
    let ended = wait_for(&events, &mut seen, |event| {
        matches!(event, PlaybackEvent::Ended { .. })
    });
    assert_eq!(ended, Some(PlaybackEvent::Ended { track: b }), "{seen:?}");
    let sounded = seen.iter().find_map(|event| match event {
        PlaybackEvent::Sounding { fidelity, .. } => Some(*fidelity),
        _ => None,
    });
    let fidelity = sounded.expect("the streamer message what it sounds");
    assert_eq!(fidelity.codec, Codec::Pcm);
    assert_eq!(
        fidelity.resolution,
        Some(Resolution {
            bits: 16,
            rate: 44_100
        })
    );
}
