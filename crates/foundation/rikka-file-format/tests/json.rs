use rikka_file_format::{DecodeError, Error, JsonFormat};
use serde::{Deserialize, Serialize};

const NOTES: JsonFormat = JsonFormat::new("rikka-notes");

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Notes {
    title: String,
    lines: Vec<String>,
}

fn notes() -> Notes {
    Notes {
        title: "plan".into(),
        lines: vec!["one".into(), "two".into()],
    }
}

#[test]
fn content_round_trips_inside_envelope() {
    let json = NOTES.encode(&notes()).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["format"], "rikka-notes");
    assert_eq!(value["version"], 1);
    assert_eq!(value["content"]["title"], "plan");
    assert!(json.ends_with("}\n"));
    assert_eq!(NOTES.decode::<Notes>(&json).unwrap(), notes());
}

#[test]
fn identity_mismatches_are_named() {
    let content = r#"{"title":"plan","lines":[]}"#;
    let cases = [
        (
            format!(r#"{{"format":"rikka-other","version":1,"content":{content}}}"#),
            DecodeError::Foreign {
                expected: "rikka-notes",
            },
        ),
        (
            r#"{"format":"rikka-other","version":1,"content":{"x":1}}"#.to_owned(),
            DecodeError::Foreign {
                expected: "rikka-notes",
            },
        ),
        (
            r#"{"title":"plan","lines":[]}"#.to_owned(),
            DecodeError::Foreign {
                expected: "rikka-notes",
            },
        ),
        (
            format!(r#"{{"format":"rikka-notes","version":2,"content":{content}}}"#),
            DecodeError::Version {
                format: "rikka-notes",
                found: 2,
            },
        ),
        (
            r#"{"content":{"later":true},"version":7,"format":"rikka-notes"}"#.to_owned(),
            DecodeError::Version {
                format: "rikka-notes",
                found: 7,
            },
        ),
    ];
    for (json, expected) in cases {
        match NOTES.decode::<Notes>(&json) {
            Err(Error::Decode(found)) => assert_eq!(found, expected, "{json}"),
            other => panic!("{json}: {other:?}"),
        }
    }
}

#[test]
fn malformed_content_reports_position() {
    let cases = [
        "{\"format\":\"rikka-notes\",\"version\":1,\n\"content\":{\"title\":3,\"lines\":[]}}",
        "{\"format\":\"rikka-notes\",\"version\":1,\n\"content\":{\"title\":\"a\",\"lines\":[],\"extra\":0}}",
        "{\"format\":\"rikka-notes\",\"version\":1,\n\"content\":{\"title\":\"a\",\"lines\":[]},\"extra\":0}",
        "{\"format\":\"rikka-notes\",\n\"content\":{\"title\":\"a\",\"lines\":[]}}",
        "{\"format\":\"rikka-notes\",\"version\":1,\n\"content\":{\"title\":\"a\",\"lines\":[]",
    ];
    for json in cases {
        match NOTES.decode::<Notes>(json) {
            Err(Error::Json { format, source }) => {
                assert_eq!(format, "rikka-notes");
                assert!(source.line() >= 2, "{json}: {source}");
            }
            other => panic!("{json}: {other:?}"),
        }
    }
    assert!(matches!(
        NOTES.decode::<Notes>("not json"),
        Err(Error::Json { .. })
    ));
}

#[test]
fn save_replaces_file_atomically() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("nested/plan.rnotes");
    NOTES.save(&path, &notes()).unwrap();
    NOTES
        .save(
            &path,
            &Notes {
                title: "after".into(),
                lines: Vec::new(),
            },
        )
        .unwrap();
    let loaded: Notes = NOTES.load(&path).unwrap();
    assert_eq!(loaded.title, "after");
    let leftovers = std::fs::read_dir(path.parent().unwrap()).unwrap().count();
    assert_eq!(leftovers, 1);
}

#[test]
fn load_reports_missing_file() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("absent.rnotes");
    match NOTES.load::<Notes>(&path) {
        Err(Error::Read { path: reported, .. }) => assert_eq!(reported, path),
        other => panic!("{other:?}"),
    }
}
