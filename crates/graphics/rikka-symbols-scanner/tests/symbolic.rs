use std::process::{Command, Output};

use rikka_test_fonts::font_path;

const SCANNER: &str = env!("CARGO_BIN_EXE_rikka-symbols-scanner");

const FONT: &str = "InterVariable.ttf";

fn symbolic(name: &str) -> (Output, Option<String>) {
    let directory = tempfile::tempdir().unwrap();
    let listing = directory.path().join("listing");
    let output = directory.path().join("icon-symbolic.svg");
    std::fs::write(&listing, "a 61\nspace 20\n").unwrap();
    let result = Command::new(SCANNER)
        .arg("symbolic")
        .arg(&listing)
        .arg(font_path(FONT))
        .arg(name)
        .arg(&output)
        .output()
        .unwrap();
    (result, std::fs::read_to_string(&output).ok())
}

fn attribute<'a>(document: &'a str, name: &str) -> &'a str {
    let start = document
        .find(&format!(" {name}=\""))
        .unwrap_or_else(|| panic!("{document} has no {name}"))
        + name.len()
        + 3;
    let length = document[start..].find('"').unwrap();
    &document[start..start + length]
}

#[test]
fn listed_icon_becomes_symbolic_svg() {
    let (result, document) = symbolic("a");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );

    let document = document.expect("document written");
    assert!(document.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
    assert_eq!(attribute(&document, "width"), "16");
    assert_eq!(attribute(&document, "height"), "16");

    let view: Vec<f32> = attribute(&document, "viewBox")
        .split(' ')
        .map(|number| number.parse().unwrap())
        .collect();
    assert_eq!(view.len(), 4);
    assert!(view[2] > 0.0 && view[2] == view[3] && view[1] == -view[3]);

    let path_data = attribute(&document, "d");
    assert!(
        path_data.starts_with('M') && path_data.ends_with('Z'),
        "{path_data}"
    );
    let coordinates: Vec<f32> = path_data
        .split(|c: char| c.is_ascii_alphabetic() || c == ' ')
        .filter(|number| !number.is_empty())
        .map(|number| number.parse().unwrap())
        .collect();
    assert!(coordinates.len() >= 6 && coordinates.len().is_multiple_of(2));
    assert!(
        coordinates
            .chunks(2)
            .any(|point| point[0] > 0.0 && point[1] < 0.0),
        "{path_data} draws nothing above the baseline"
    );
}

#[test]
fn unknown_or_empty_icon_writes_nothing() {
    for (name, named) in [
        ("nothing", "no icon named `nothing`"),
        ("space", "no outline"),
    ] {
        let (result, document) = symbolic(name);
        let stderr = String::from_utf8_lossy(&result.stderr);

        assert!(!result.status.success(), "{name} was drawn");
        assert!(stderr.contains(named), "{stderr} does not say {named}");
        assert_eq!(document, None);
    }
}
