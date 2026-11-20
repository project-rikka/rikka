use std::time::{Duration, Instant};

use rikka_hifi::{Codec, Event, Failure, OutputPath, Source, SourceFormat, Stage, Verdict};

use crate::support::{DSD64, describe, dsd, hifi, last_verdict, tone, wait_for};

fn silent(seen: &[Event]) -> bool {
    let silent = seen.iter().any(|event| {
        matches!(
            event,
            Event::Failed {
                failure: Failure::Silent(_),
                ..
            }
        )
    });
    if silent {
        eprintln!("no PipeWire to play through: the shared test is skipped");
    }
    silent
}

#[test]
fn shared_output_is_gapless_and_reports_graph() {
    let hifi = hifi();
    let mut events = hifi.events();
    hifi.set_volume(0.1);
    let first = hifi.play(tone(44_100, 1.0, 440.0));
    let second = hifi.queue(tone(44_100, 1.0, 660.0));
    let mut seen = Vec::new();
    let started = wait_for(&mut events, &mut seen, |event| {
        matches!(event, Event::Started(_) | Event::Failed { .. })
    });
    if silent(&seen) {
        return;
    }
    assert_eq!(started, Some(Event::Started(first)));
    let began = Instant::now();
    assert!(
        wait_for(&mut events, &mut seen, |event| *event
            == Event::Advanced {
                from: first,
                to: second
            })
        .is_some(),
        "the queued source never followed"
    );
    let advanced = Instant::now();
    assert!(
        wait_for(&mut events, &mut seen, |event| *event
            == Event::Ended(second))
        .is_some(),
        "the second source never ended"
    );
    let ended = Instant::now();
    describe(&seen);
    assert!(
        advanced - began >= Duration::from_millis(800),
        "{:?}",
        advanced - began
    );
    assert!(
        ended - advanced >= Duration::from_millis(800),
        "{:?}",
        ended - advanced
    );
    describe(&seen);

    let linked = seen.iter().rev().find_map(|event| match event {
        Event::Path(path) => match &path.output {
            OutputPath::Shared(shared) => shared.sink.clone(),
            OutputPath::Exclusive(_) => None,
        },
        _ => None,
    });
    let sink = linked.expect("the path named the sink the stream was linked to");
    assert!(!sink.name.is_empty());
    assert!(
        sink.rate.is_some(),
        "the sink's rate was read from its format"
    );
    assert!(last_verdict(&seen).is_some_and(|verdict| verdict >= Verdict::HighQuality));

    let positions: Vec<Duration> = seen
        .iter()
        .filter_map(|event| match event {
            Event::Position { source, at } if *source == first => Some(*at),
            _ => None,
        })
        .collect();
    assert!(positions.windows(2).all(|pair| pair[0] <= pair[1]));
    assert!(
        positions
            .last()
            .is_some_and(|at| *at <= Duration::from_millis(1_050))
    );
}

#[test]
fn seek_moves_position() {
    let hifi = hifi();
    let mut events = hifi.events();
    hifi.set_volume(0.1);
    let id = hifi.play(tone(48_000, 4.0, 330.0));
    let mut seen = Vec::new();
    wait_for(&mut events, &mut seen, |event| {
        matches!(event, Event::Started(_) | Event::Failed { .. })
    });
    if silent(&seen) {
        return;
    }
    hifi.seek(Duration::from_secs(3));
    let jumped = wait_for(
        &mut events,
        &mut seen,
        |event| matches!(event, Event::Position { source, at } if *source == id && *at >= Duration::from_secs(3)),
    );
    assert!(jumped.is_some(), "the position never moved past the seek");
    let ended = wait_for(&mut events, &mut seen, |event| *event == Event::Ended(id));
    assert!(ended.is_some(), "the seeked source never ended");
}

#[test]
fn pushed_pcm_reports_declared_encoding() {
    let hifi = hifi();
    let mut events = hifi.events();
    hifi.set_volume(0.1);
    let format = SourceFormat::pcm(44_100, 2)
        .codec(Codec::Vorbis)
        .bitrate(320_000);
    let (source, writer) = Source::pushed(format);
    let id = hifi.play(source);
    let producer = std::thread::spawn(move || {
        let mut block = Vec::with_capacity(4410 * 2);
        for chunk in 0..10 {
            block.clear();
            for frame in 0..4410 {
                let at = (chunk * 4410 + frame) as f32 / 44_100.0;
                let value = (std::f32::consts::TAU * 550.0 * at).sin() * 0.1;
                block.extend([value, value]);
            }
            if writer.write(&block).is_err() {
                return;
            }
        }
        writer.finish();
    });
    let mut seen = Vec::new();
    let ended = wait_for(&mut events, &mut seen, |event| {
        matches!(event, Event::Ended(_) | Event::Failed { .. })
    });
    producer.join().expect("the producer finished");
    if silent(&seen) {
        return;
    }
    describe(&seen);
    assert_eq!(ended, Some(Event::Ended(id)));
    assert_eq!(last_verdict(&seen), Some(Verdict::Lossy));
    assert!(seen.iter().any(|event| matches!(
        event,
        Event::Path(path) if path.stages.iter().any(|stage| matches!(stage, Stage::Volume { .. }))
    )));
}

#[test]
fn shared_output_converts_dsd_to_pcm() {
    let hifi = hifi();
    let mut events = hifi.events();
    hifi.set_volume(0.1);
    let id = hifi.play(dsd());
    let mut seen = Vec::new();
    let ended = wait_for(&mut events, &mut seen, |event| {
        matches!(event, Event::Ended(_) | Event::Failed { .. })
    });
    if silent(&seen) {
        return;
    }
    describe(&seen);
    assert_eq!(ended, Some(Event::Ended(id)));
    let converted = seen.iter().rev().find_map(|event| match event {
        Event::Path(path) => path.stages.iter().find_map(|stage| match stage {
            Stage::DsdToPcm { from, to } => Some((*from, *to)),
            _ => None,
        }),
        _ => None,
    });
    assert!(
        converted.is_some_and(|(from, to)| from == DSD64 && to >= 88_200),
        "{converted:?}"
    );
}

#[test]
fn unqueue_withdraws_successor_before_seam() {
    let hifi = hifi();
    let mut events = hifi.events();
    hifi.set_volume(0.1);
    let first = hifi.play(tone(44_100, 1.0, 440.0));
    let second = hifi.queue(tone(44_100, 1.0, 660.0));
    let mut seen = Vec::new();
    wait_for(&mut events, &mut seen, |event| {
        matches!(event, Event::Started(_) | Event::Failed { .. })
    });
    if silent(&seen) {
        return;
    }
    let began = Instant::now();
    wait_for(
        &mut events,
        &mut seen,
        |event| matches!(event, Event::Position { source, at } if *source == first && *at >= Duration::from_millis(400)),
    )
    .expect("the first source played for a while");
    hifi.unqueue();
    hifi.unqueue();
    let ended = wait_for(&mut events, &mut seen, |event| {
        matches!(event, Event::Ended(_) | Event::Advanced { .. })
    });
    let played = began.elapsed();
    describe(&seen);
    assert_eq!(ended, Some(Event::Ended(first)));
    assert!(
        played >= Duration::from_millis(850),
        "ended after {played:?}"
    );
    assert!(!seen.iter().any(|event| matches!(
        event,
        Event::Started(id) | Event::Position { source: id, .. } if *id == second
    )));
}

#[test]
fn requeued_source_plays_after_unqueue() {
    let hifi = hifi();
    let mut events = hifi.events();
    hifi.set_volume(0.1);
    let first = hifi.play(tone(44_100, 0.6, 440.0));
    let withdrawn = hifi.queue(tone(44_100, 0.6, 660.0));
    hifi.unqueue();
    let third = hifi.queue(tone(44_100, 0.6, 550.0));
    let mut seen = Vec::new();
    wait_for(&mut events, &mut seen, |event| {
        matches!(event, Event::Started(_) | Event::Failed { .. })
    });
    if silent(&seen) {
        return;
    }
    let advanced = wait_for(&mut events, &mut seen, |event| {
        matches!(event, Event::Advanced { .. })
    });
    assert_eq!(
        advanced,
        Some(Event::Advanced {
            from: first,
            to: third
        })
    );
    let ended = wait_for(&mut events, &mut seen, |event| {
        matches!(event, Event::Ended(_))
    });
    describe(&seen);
    assert_eq!(ended, Some(Event::Ended(third)));
    assert!(!seen.iter().any(|event| matches!(
        event,
        Event::Started(id) | Event::Position { source: id, .. } if *id == withdrawn
    )));
}
