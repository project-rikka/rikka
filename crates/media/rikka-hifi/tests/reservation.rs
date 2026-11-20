use std::time::{Duration, Instant};

use rikka_hifi::{Event, Output, OutputPath, OutputState, Refusal};

use crate::support::card::{self, ask_release, claimed, holder};
use crate::support::{describe, hifi_as, tone, wait_for};

const YIELDING: &str = "Rikka Hifi Yielding";
const STALLED: &str = "Rikka Hifi Stalled Holder";

#[test]
fn release_request_moves_playback_to_shared() {
    let _card = card::lock();
    let Some(device) = card::device() else {
        return;
    };
    let hifi = hifi_as(YIELDING);
    let mut events = hifi.events();
    hifi.set_volume(0.05);
    hifi.set_output(Output::Exclusive(device.id.clone()));
    let id = hifi.play(tone(44_100, 2.5, 440.0));
    let mut seen = Vec::new();
    if !claimed(&mut seen, &mut events) {
        return;
    }
    let Some(Event::Position { at: before, .. }) = wait_for(
        &mut events,
        &mut seen,
        |event| matches!(event, Event::Position { at, .. } if *at >= Duration::from_millis(500)),
    ) else {
        panic!("the exclusive source never played half a second");
    };

    let requested_at = ask_release(&card::bus(), device.card, i32::MAX);
    assert_eq!(requested_at.ok(), Some(true), "the card was not given up");
    assert_ne!(holder(device.card).as_deref(), Some(YIELDING));

    let released = wait_for(&mut events, &mut seen, |event| {
        matches!(event, Event::Output(OutputState::FellBack { .. }))
    });
    assert!(
        matches!(
            released,
            Some(Event::Output(OutputState::FellBack {
                reason: Refusal::Released,
                ..
            }))
        ),
        "{released:?}"
    );
    let moved = wait_for(
        &mut events,
        &mut seen,
        |event| matches!(event, Event::Path(path) if matches!(path.output, OutputPath::Shared(_))),
    );
    assert!(moved.is_some(), "playback never moved to the shared graph");
    let Some(Event::Position { at: after, .. }) = wait_for(&mut events, &mut seen, |event| {
        matches!(event, Event::Position { .. })
    }) else {
        panic!("no position after the hand-over");
    };
    assert!(after >= before, "went back from {before:?} to {after:?}");
    let ended = wait_for(&mut events, &mut seen, |event| *event == Event::Ended(id));
    describe(&seen);
    assert!(
        ended.is_some(),
        "the source never ended after the hand-over"
    );
}

#[test]
fn unresponsive_holder_refuses_claim() {
    let _card = card::lock();
    let Some(device) = card::device() else {
        return;
    };
    let Some(stalled) = card::stall(device.card, STALLED) else {
        return;
    };
    let hifi = hifi_as(YIELDING);
    let mut events = hifi.events();
    hifi.set_volume(0.05);
    hifi.set_output(Output::Exclusive(device.id.clone()));
    let requested_at = Instant::now();
    hifi.play(tone(44_100, 0.3, 440.0));
    let mut seen = Vec::new();
    let refusal = wait_for(&mut events, &mut seen, |event| {
        matches!(event, Event::Output(_))
    });
    let waited = requested_at.elapsed();
    describe(&seen);
    assert_eq!(
        refusal,
        Some(Event::Output(OutputState::FellBack {
            device: device.id.clone(),
            reason: Refusal::Busy {
                holder: Some(STALLED.to_owned()),
            },
        }))
    );
    assert!(
        waited < card::STALLED_REPLY,
        "the refusal waited for the stalled holder's reply ({waited:?})"
    );
    drop(stalled);
}

#[test]
fn racing_players_all_play() {
    let _card = card::lock();
    let Some(device) = card::device() else {
        return;
    };
    let mut racers: Vec<_> = (0..3)
        .map(|racer| {
            let hifi = hifi_as(&format!("Rikka Hifi Racer {racer}"));
            let events = hifi.events();
            hifi.set_volume(0.05);
            hifi.set_output(Output::Exclusive(device.id.clone()));
            (hifi, events)
        })
        .collect();
    let played: Vec<_> = racers
        .iter()
        .map(|(hifi, _)| hifi.play(tone(44_100, 0.5, 440.0)))
        .collect();
    for ((_, events), id) in racers.iter_mut().zip(played) {
        let mut seen = Vec::new();
        let answered = wait_for(events, &mut seen, |event| matches!(event, Event::Output(_)));
        assert!(
            matches!(
                answered,
                Some(Event::Output(
                    OutputState::Exclusive(_)
                        | OutputState::FellBack {
                            reason: Refusal::Busy { .. },
                            ..
                        }
                ))
            ),
            "a racer was answered {answered:?}"
        );
        let ended = wait_for(events, &mut seen, |event| *event == Event::Ended(id));
        describe(&seen);
        assert!(ended.is_some(), "a racer never finished");
    }
}
