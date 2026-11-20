use serde::{Deserialize, Serialize};

use rikka_portal::Outcome;

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
struct Answer {
    chosen: Vec<String>,
}

#[test]
fn answer_is_stored_beside_outcome() {
    let outcome = Outcome::Chosen(Answer {
        chosen: vec!["DP-1".to_owned()],
    });
    let mut buffer = Vec::new();
    outcome.write(&mut buffer).unwrap();

    assert_eq!(Outcome::read(buffer.as_slice()).unwrap(), outcome);
    let buffer = String::from_utf8(buffer).unwrap();
    assert!(buffer.contains(r#""outcome":"chosen""#), "{buffer}");
    assert!(buffer.contains(r#""chosen":["DP-1"]"#), "{buffer}");
}

#[test]
fn denial_has_no_extra_output() {
    let mut buffer = Vec::new();
    Outcome::<Answer>::Cancelled.write(&mut buffer).unwrap();

    assert_eq!(buffer, b"{\"outcome\":\"cancelled\"}\n");
    assert_eq!(
        Outcome::<Answer>::read(buffer.as_slice()).unwrap(),
        Outcome::Cancelled
    );
}
