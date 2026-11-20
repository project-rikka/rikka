use rikka_prompt::{Facts, Prompt};

fn facts() -> Facts {
    let mut facts = Facts::new();
    facts.set("project", "/home/someone/project");
    facts
}

#[test]
fn front_matter_parameters_fill_body_and_tool() {
    let prompt = Prompt::read(
        "read",
        "\u{feff}+++\nname = \"Read\"\ndeferred = true\n[params]\npath = \"The file.\"\n[limits]\nmax_lines = 2000\n+++\n\nUp to {{max_lines}} lines of {{project}}; write +++ and `{{ key }}` as they are.\n",
    )
    .expect("document should parse");

    assert_eq!(prompt.name(), "Read");
    assert_eq!(prompt.stem(), "read");
    assert!(prompt.is_deferred());
    assert_eq!(prompt.limit("max_lines").unwrap(), 2000);
    assert_eq!(prompt.param("path", &facts()).unwrap(), "The file.");
    assert_eq!(prompt.params().collect::<Vec<_>>(), ["path"]);
    assert_eq!(
        prompt.body(&facts()).unwrap(),
        "Up to 2000 lines of /home/someone/project; write +++ and `{{ key }}` as they are."
    );
}

#[test]
fn document_without_front_matter_uses_file_name() {
    let prompt = Prompt::read("system", "You are the agent.").expect("prompt should parse");

    assert_eq!(prompt.name(), "system");
    assert!(!prompt.is_deferred());
    assert_eq!(prompt.body(&facts()).unwrap(), "You are the agent.");
}

#[test]
fn invalid_document_is_rejected() {
    let unsupplied = Prompt::read("read", "Reads {{whatever}}.")
        .expect("document should parse")
        .body(&facts())
        .expect_err("nothing supplies it");
    assert!(unsupplied.to_string().contains("whatever"));

    let prompt = Prompt::read("read", "+++\n[params]\npath = \"The file.\"\n+++\nReads.")
        .expect("document should parse");
    assert!(prompt.param("offset", &facts()).is_err());
    assert!(prompt.limit("max_lines").is_err());

    for (text, mentioned) in [
        ("+++\nname = \"Read\"\n\nReads a file.\n", "never closed"),
        ("+++\ndeferred = \"yes\"\n+++\nFetches.\n", "deferred"),
        (
            "+++\n[limits]\ntimeout_ms = \"a while\"\n+++\nRuns.\n",
            "timeout_ms",
        ),
    ] {
        let error = Prompt::read("broken", text).expect_err("the document is malformed");
        assert!(error.to_string().contains(mentioned), "{error}");
    }
}
