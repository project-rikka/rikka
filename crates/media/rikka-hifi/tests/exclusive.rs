use std::time::{Duration, Instant};

use rikka_hifi::{Event, Output, OutputPath, OutputState, Refusal, Stage, Verdict, VolumeControl};

use crate::support::card::{self, OwnerWatch, claimed, holder};
use crate::support::{describe, dsd, hifi_as, last_verdict, tone, wait_for};

const PLAYER: &str = "Rikka Hifi Exclusive";
const PLAYING_SPAN: Duration = Duration::from_millis(300);
const PAUSED_SPAN: Duration = Duration::from_millis(700);
const RELEASE_TIMEOUT: Duration = Duration::from_secs(2);
#[test]
fn exclusive_output_is_bit_perfect() {
    let _card = card::lock();
    let Some(device) = card::device() else {
        return;
    };
    eprintln!("playing on {device:#?}");
    let hifi = hifi_as(PLAYER);
    let mut events = hifi.events();
    hifi.set_volume(0.05);
    hifi.set_output(Output::Exclusive(device.id.clone()));
    let id = hifi.play(tone(44_100, 1.5, 440.0));
    let mut seen = Vec::new();
    if !claimed(&mut seen, &mut events) {
        return;
    }
    let started = wait_for(&mut events, &mut seen, |event| {
        matches!(event, Event::Started(_))
    });
    assert_eq!(started, Some(Event::Started(id)));
    let began = Instant::now();
    std::thread::sleep(PLAYING_SPAN);
    assert_eq!(holder(device.card).as_deref(), Some(PLAYER));
    let ended = wait_for(&mut events, &mut seen, |event| *event == Event::Ended(id));
    let played = began.elapsed();
    describe(&seen);
    assert!(ended.is_some(), "the exclusive source never ended");
    assert!(
        played >= Duration::from_millis(1_300),
        "ended after {played:?}"
    );

    let path = seen
        .iter()
        .rev()
        .find_map(|event| match event {
            Event::Path(path) => Some(path.clone()),
            _ => None,
        })
        .expect("a signal path was reported");
    let OutputPath::Exclusive(output) = &path.output else {
        panic!("the path was not exclusive: {path:#?}");
    };
    assert_eq!(output.rate, 44_100);
    let hardware = path.stages.iter().any(|stage| {
        matches!(
            stage,
            Stage::Volume {
                by: VolumeControl::Hardware,
                ..
            }
        )
    });
    let expected = if hardware {
        Verdict::BitPerfect
    } else {
        Verdict::HighQuality
    };
    assert_eq!(last_verdict(&seen), Some(expected));

    let mut owners = OwnerWatch::new(device.card);
    drop(hifi);
    assert!(
        owners.wait_until(RELEASE_TIMEOUT, || holder(device.card).as_deref()
            != Some(PLAYER)),
        "the card was not handed back"
    );
}

#[test]
fn rate_change_reopens_card() {
    let _card = card::lock();
    let Some(device) = card::device() else {
        return;
    };
    let hifi = hifi_as(PLAYER);
    let mut events = hifi.events();
    hifi.set_volume(0.05);
    hifi.set_output(Output::Exclusive(device.id.clone()));
    let first = hifi.play(tone(44_100, 1.0, 440.0));
    let second = hifi.queue(tone(96_000, 1.0, 550.0));
    let mut seen = Vec::new();
    if !claimed(&mut seen, &mut events) {
        return;
    }

    let rival = hifi_as("Rikka Hifi Rival");
    let mut rivalling = rival.events();
    rival.set_volume(0.05);
    rival.set_output(Output::Exclusive(device.id.clone()));
    rival.play(tone(44_100, 0.5, 330.0));
    let mut rival_seen = Vec::new();
    let refused = wait_for(&mut rivalling, &mut rival_seen, |event| {
        matches!(event, Event::Output(_))
    });
    describe(&rival_seen);
    assert!(
        matches!(
            refused,
            Some(Event::Output(OutputState::FellBack {
                reason: Refusal::Busy { .. },
                ..
            }))
        ),
        "the rival got {refused:?}"
    );
    drop(rival);

    let advanced = wait_for(&mut events, &mut seen, |event| {
        *event
            == Event::Advanced {
                from: first,
                to: second,
            }
    });
    assert!(advanced.is_some(), "the 96 kHz source never followed");
    let ended = wait_for(&mut events, &mut seen, |event| {
        *event == Event::Ended(second)
    });
    describe(&seen);
    assert!(ended.is_some());
    let rates: Vec<u32> = seen
        .iter()
        .filter_map(|event| match event {
            Event::Path(path) => match &path.output {
                OutputPath::Exclusive(output) => Some(output.rate),
                OutputPath::Shared(_) => None,
            },
            _ => None,
        })
        .collect();
    assert!(
        rates.contains(&44_100) && rates.contains(&96_000),
        "{rates:?}"
    );
    assert!(
        !seen
            .iter()
            .any(|event| matches!(event, Event::Output(OutputState::FellBack { .. })))
    );
}

#[test]
fn pause_keeps_card_and_position() {
    let _card = card::lock();
    let Some(device) = card::device() else {
        return;
    };
    let hifi = hifi_as(PLAYER);
    let mut events = hifi.events();
    hifi.set_volume(0.05);
    hifi.set_output(Output::Exclusive(device.id.clone()));
    let id = hifi.play(tone(48_000, 2.0, 440.0));
    let mut seen = Vec::new();
    if !claimed(&mut seen, &mut events) {
        return;
    }
    wait_for(
        &mut events,
        &mut seen,
        |event| matches!(event, Event::Position { at, .. } if *at >= Duration::from_millis(500)),
    )
    .expect("the source played half a second");
    hifi.pause();
    wait_for(&mut events, &mut seen, |event| *event == Event::Paused).expect("paused");
    let paused_at = seen
        .iter()
        .rev()
        .find_map(|event| match event {
            Event::Position { at, .. } => Some(*at),
            _ => None,
        })
        .expect("a position before the pause");
    std::thread::sleep(PAUSED_SPAN);
    assert_eq!(holder(device.card).as_deref(), Some(PLAYER));
    hifi.resume();
    let resumed = wait_for(&mut events, &mut seen, |event| {
        matches!(event, Event::Position { .. })
    });
    let Some(Event::Position { at, .. }) = resumed else {
        panic!("no position after resuming");
    };
    assert!(
        at >= paused_at && at <= paused_at + Duration::from_millis(400),
        "paused at {paused_at:?}, resumed at {at:?}"
    );
    assert!(wait_for(&mut events, &mut seen, |event| *event == Event::Ended(id)).is_some());
}

#[test]
fn dsd_plays_native_or_dop() {
    let _card = card::lock();
    let Some(device) = card::device() else {
        return;
    };
    let hifi = hifi_as(PLAYER);
    let mut events = hifi.events();
    hifi.set_volume(0.05);
    hifi.set_output(Output::Exclusive(device.id.clone()));
    let id = hifi.play(dsd());
    let mut seen = Vec::new();
    if !claimed(&mut seen, &mut events) {
        return;
    }
    let ended = wait_for(&mut events, &mut seen, |event| *event == Event::Ended(id));
    describe(&seen);
    assert!(ended.is_some(), "the DSD source never ended");
    let path = seen
        .iter()
        .rev()
        .find_map(|event| match event {
            Event::Path(path) => Some(path.clone()),
            _ => None,
        })
        .expect("a signal path was reported");
    let OutputPath::Exclusive(output) = &path.output else {
        panic!("the DSD path was not exclusive: {path:#?}");
    };
    let dsd = device.dsd();
    if !dsd.native.is_empty() {
        assert!(output.format.is_dsd(), "{path:#?}");
        assert_eq!(path.verdict(), Verdict::BitPerfect);
    } else if !dsd.dop.is_empty() {
        assert!(output.dop, "{path:#?}");
    } else {
        assert!(
            path.stages
                .iter()
                .any(|stage| matches!(stage, Stage::DsdToPcm { .. }))
        );
    }
}
