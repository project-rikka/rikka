use std::collections::BTreeSet;

use rikka_nix::{Distribution, Document, Error, Module};

const CATALOG: &str = r#"{
  "components": {
    "base": {"name": "Rikka desktop", "summary": "The session", "offer": "required", "version": "0.1.0", "installed": true},
    "browser": {"name": "Browser", "summary": "Browse the web", "offer": "selected", "version": "0.1.0", "installed": true},
    "modeler": {"name": "Modeler", "summary": "Model in 3D", "offer": "offered", "version": "0.2.0", "installed": false}
  }
}"#;

fn distribution() -> Distribution {
    Distribution::parse(CATALOG).expect("the fixture parses")
}

#[test]
fn apps_module_round_trips_choices() {
    let distribution = distribution();
    let mut apps = Module::Apps.blank();
    apps.set("rikka.apps.browser.enable", true)
        .expect("it sets");
    assert_eq!(
        distribution.chosen(&apps).expect("it reads"),
        BTreeSet::from(["browser".to_owned()])
    );

    let chosen = BTreeSet::from(["modeler".to_owned(), "base".to_owned()]);
    distribution.choose(&mut apps, &chosen).expect("it writes");
    let written = Document::parse(apps.source()).expect("output should parse as Nix");
    assert_eq!(
        written.get::<bool>("rikka.apps.modeler.enable"),
        Ok(Some(true))
    );
    assert_eq!(written.get::<bool>("rikka.apps.browser.enable"), Ok(None));
    assert_eq!(written.get::<bool>("rikka.apps.base.enable"), Ok(None));
    assert_eq!(
        distribution.chosen(&written).expect("it reads"),
        BTreeSet::from(["modeler".to_owned()])
    );
}

#[test]
fn malformed_catalog_is_refused() {
    for json in ["", "{}", r#"{"components": {"base": {"name": "Rikka"}}}"#] {
        assert!(
            matches!(Distribution::parse(json), Err(Error::Catalog(_))),
            "{json}"
        );
    }
}
