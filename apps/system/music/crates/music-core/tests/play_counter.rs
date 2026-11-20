use std::time::Duration;

use music_core::{Id, PlayCounter, PlaybackEvent};

#[derive(Clone, Copy)]
enum Step {
    Load,
    Length(u64),
    Play(u64),
    Run(u64, u64),
    Seek(u64),
    Pause(u64),
    Other,
    Advance,
}

fn events(track: &Id, steps: &[Step]) -> Vec<PlaybackEvent> {
    let at = |seconds: u64| Duration::from_secs(seconds);
    let mut message = Vec::new();
    for step in steps {
        match *step {
            Step::Load => message.push(PlaybackEvent::Loading {
                track: track.clone(),
            }),
            Step::Length(seconds) => message.push(PlaybackEvent::Length {
                track: track.clone(),
                length: at(seconds),
            }),
            Step::Play(seconds) => message.push(PlaybackEvent::Playing {
                track: track.clone(),
                at: at(seconds),
            }),
            Step::Run(from, to) => {
                message.extend((from * 2..=to * 2).map(|half| PlaybackEvent::Position {
                    track: track.clone(),
                    at: Duration::from_millis(half * 500),
                }));
            }
            Step::Seek(seconds) => message.push(PlaybackEvent::Position {
                track: track.clone(),
                at: at(seconds),
            }),
            Step::Pause(seconds) => message.push(PlaybackEvent::Paused {
                track: track.clone(),
                at: at(seconds),
            }),
            Step::Other => message.push(PlaybackEvent::Loading {
                track: Id::new("other"),
            }),
            Step::Advance => message.push(PlaybackEvent::Advanced {
                from: track.clone(),
                to: track.clone(),
            }),
        }
    }
    message
}

#[test]
fn play_counts_after_half_or_four_minutes() {
    use Step::{Advance, Length, Load, Other, Pause, Play, Run, Seek};
    let cases: [(&str, &[Step], usize); 10] = [
        (
            "half of a short song",
            &[Load, Length(200), Play(0), Run(0, 101)],
            1,
        ),
        (
            "just under half",
            &[Load, Length(200), Play(0), Run(0, 99)],
            0,
        ),
        (
            "four minutes of a long song",
            &[Load, Length(600), Play(0), Run(0, 241)],
            1,
        ),
        (
            "four minutes of a song of unknown length",
            &[Load, Play(0), Run(0, 241)],
            1,
        ),
        (
            "a seek ahead is not listening",
            &[Load, Length(200), Play(0), Seek(150), Run(150, 199)],
            0,
        ),
        (
            "time paused is not listening",
            &[
                Load,
                Length(200),
                Play(0),
                Run(0, 60),
                Pause(60),
                Seek(140),
                Run(140, 150),
            ],
            0,
        ),
        (
            "a song shorter than thirty seconds never counts",
            &[Load, Length(20), Play(0), Run(0, 20)],
            0,
        ),
        (
            "playing on after counting adds nothing",
            &[
                Load,
                Length(200),
                Play(0),
                Run(0, 200),
                Seek(0),
                Run(0, 200),
            ],
            1,
        ),
        (
            "playing the song again counts again",
            &[
                Load,
                Length(200),
                Play(0),
                Run(0, 120),
                Other,
                Load,
                Length(200),
                Play(0),
                Run(0, 120),
            ],
            2,
        ),
        (
            "a gapless advance into the song again counts again",
            &[
                Load,
                Length(200),
                Play(0),
                Run(0, 120),
                Advance,
                Length(200),
                Run(0, 120),
            ],
            2,
        ),
    ];
    let track = Id::new("song");
    for (case, steps, expected) in cases {
        let mut counter = PlayCounter::new();
        let counted: Vec<Id> = events(&track, steps)
            .iter()
            .filter_map(|event| counter.observe(event))
            .collect();
        assert_eq!(counted.len(), expected, "{case}");
        assert!(counted.iter().all(|id| *id == track), "{case}");
    }
}
