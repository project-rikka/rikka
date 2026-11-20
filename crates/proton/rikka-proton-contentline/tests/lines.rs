use rikka_proton_contentline::{
    ContentLine, Error, Parameter, escape_text, read_lines, split_unescaped, unescape_text,
};

#[test]
fn unfolding_joins_continuations() {
    let text = "DESCRIPTION:This is a lo\r\n ng description\r\n\t that exists on a long line.\r\nSUMMARY:x\n";
    let lines = read_lines(text).unwrap();

    assert_eq!(lines.len(), 2);
    assert_eq!(
        lines[0].value(),
        "This is a long description that exists on a long line."
    );
    assert_eq!(lines[1].name(), "SUMMARY");
}

#[test]
fn folding_respects_75_octets() {
    let value = "é".repeat(100);
    let line = ContentLine::new("SUMMARY", value.clone());
    let output = line.to_string();

    for physical in output.split("\r\n").filter(|physical| !physical.is_empty()) {
        assert!(physical.len() <= 75, "{} octets", physical.len());
    }
    let reread = read_lines(&output).unwrap();
    assert_eq!(reread[0].value(), value);
}

#[test]
fn parameters_quote_and_caret_encode() {
    let cases: &[(&str, &str, &[&str])] = &[
        ("ATTENDEE;CN=\"Doe, John\":mailto:j@x", "CN", &["Doe, John"]),
        ("TEL;TYPE=work,voice:+1", "TYPE", &["work", "voice"]),
        ("TEL;TYPE=work;TYPE=cell:+1", "TYPE", &["work", "cell"]),
        ("TEL;WORK;VOICE:+1", "TYPE", &["WORK", "VOICE"]),
        (
            "X-ADDR;LABEL=\"123 Main^nApt ^'4^'^^\":x",
            "LABEL",
            &["123 Main\nApt \"4\"^"],
        ),
        ("X;P=a^b:v", "P", &["a^b"]),
    ];
    for (text, name, values) in cases {
        let line = ContentLine::parse(text).unwrap();
        let parameter = line.parameter(name).unwrap();
        assert_eq!(parameter.values(), *values, "{text}");

        let reparsed = ContentLine::parse(line.unfolded().as_str()).unwrap();
        assert_eq!(reparsed.parameter(name), Some(parameter), "{text}");
    }
}

#[test]
fn names_are_case_insensitive() {
    let line = ContentLine::parse("item1.email;type=INTERNET:a@b.c").unwrap();

    assert_eq!(line.group(), Some("item1"));
    assert_eq!(line.name(), "EMAIL");
    assert!(line.is("Email"));
    assert_eq!(line.value(), "a@b.c");
    assert_eq!(line.unfolded(), "item1.EMAIL;TYPE=INTERNET:a@b.c");
}

#[test]
fn values_keep_later_colons() {
    let line = ContentLine::parse("URL:https://example.com:8080/a;b").unwrap();
    assert_eq!(line.value(), "https://example.com:8080/a;b");
}

#[test]
fn text_escaping_round_trips() {
    let cases = [
        ("plain", "plain"),
        ("a, b; c", "a\\, b\\; c"),
        ("back\\slash", "back\\\\slash"),
        ("two\nlines", "two\\nlines"),
        ("crlf\r\nline", "crlf\\nline"),
    ];
    for (text, escaped) in cases {
        assert_eq!(escape_text(text), escaped);
        assert_eq!(unescape_text(escaped), text.replace("\r\n", "\n"));
    }
    assert_eq!(unescape_text("UPPER\\Nnewline"), "UPPER\nnewline");
}

#[test]
fn split_honours_escaped_separators() {
    assert_eq!(
        split_unescaped("Doe;John\\;Jr;;Dr.", ';'),
        vec!["Doe", "John\\;Jr", "", "Dr."]
    );
    assert_eq!(split_unescaped("a\\\\,b", ','), vec!["a\\\\", "b"]);
}

#[test]
fn rejects_malformed_lines_with_number() {
    let cases = [
        (
            "BEGIN:VCARD\r\nNOVALUE\r\n",
            Error::MissingValue { line: 2 },
        ),
        ("X;P=\"open:value\r\n", Error::UnclosedQuote { line: 1 }),
        (":value\r\n", Error::MissingName { line: 1 }),
        ("X;=v:value\r\n", Error::MissingParameterName { line: 1 }),
        (
            "BAD NAME:value\r\n",
            Error::InvalidName {
                line: 1,
                found: ' ',
            },
        ),
    ];
    for (text, expected) in cases {
        assert_eq!(read_lines(text), Err(expected), "{text:?}");
    }
}

#[test]
fn set_parameter_replaces_previous() {
    let line = ContentLine::new("ATTENDEE", "mailto:a@b.c")
        .with_parameter(Parameter::new("PARTSTAT", "NEEDS-ACTION"))
        .with_parameter(Parameter::new("partstat", "ACCEPTED"));

    assert_eq!(line.parameters().len(), 1);
    assert_eq!(line.parameter_value("PARTSTAT"), Some("ACCEPTED"));
}
