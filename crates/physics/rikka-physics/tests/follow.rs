use rikka_physics::{Follow, Pace, Step};

const IDLE: Pace = Pace::Stopped { idle: 1.0 / 60.0 };

#[test]
fn follow_advances_while_playing_and_stopped() {
    let follow = Follow::default();
    assert_eq!(follow.step(None, 2.0, IDLE), Step::Seek);
    assert_eq!(
        follow.step(Some(2.0), 2.5, Pace::Playing),
        Step::Advance(0.5)
    );
    assert_eq!(
        follow.step(Some(2.0), 3.0, Pace::Playing),
        Step::Advance(1.0)
    );
    assert_eq!(follow.step(Some(2.0), 4.0, Pace::Playing), Step::Seek);
    assert_eq!(
        follow.step(Some(2.0), 1.9, Pace::Playing),
        Step::Advance(0.0),
        "a playing clock that slips back to meet its sound waits for the simulation"
    );
    assert_eq!(
        follow.step(Some(9.0), 0.0, Pace::Playing),
        Step::Seek,
        "a repeat lap starts over"
    );
    assert_eq!(follow.step(Some(2.0), 2.0, IDLE), Step::Advance(1.0 / 60.0));
    assert_eq!(follow.step(Some(2.0), 1.9, IDLE), Step::Advance(1.0 / 60.0));
    assert_eq!(follow.step(Some(2.0), 4.0, IDLE), Step::Seek);

    let strict = Follow::default().max_jump(0.1).max_backward_jitter(0.0);
    assert_eq!(strict.step(Some(2.0), 2.5, Pace::Playing), Step::Seek);
    assert_eq!(strict.step(Some(2.0), 1.99, Pace::Playing), Step::Seek);
    assert_eq!(
        strict.step(Some(2.0), 2.0625, Pace::Playing),
        Step::Advance(0.0625)
    );
}
