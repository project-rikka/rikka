use rikka_xdg::{Exec, FieldCodes};

fn codes<'a>(paths: &'a [&'a str], uris: &'a [&'a str]) -> FieldCodes<'a> {
    FieldCodes {
        paths,
        uris,
        name: "Test Application",
        icon: Some("test-icon"),
        location: Some("/x/test.desktop"),
    }
}

fn argv(exec: &str) -> Vec<String> {
    Exec::expand(exec, &codes(&[], &[]))
}

#[test]
fn quotes_group_without_shell_expansion() {
    assert_eq!(
        argv(r#""/opt/My App/bin/app" --flag"#),
        ["/opt/My App/bin/app", "--flag"]
    );
    assert_eq!(
        argv(r#"app "a \"quoted\" word""#),
        ["app", r#"a "quoted" word"#]
    );
    assert_eq!(
        argv(r#"app "$HOME/x" "`whoami`""#),
        ["app", "$HOME/x", "`whoami`"],
        "nothing is expanded and nothing is run"
    );
}

#[test]
fn file_codes_expand_without_empty_arguments() {
    assert_eq!(argv("gedit %U"), ["gedit"]);
    assert_eq!(argv("gedit %f %F %u"), ["gedit"]);
    assert_eq!(argv("gedit --file=%f"), ["gedit", "--file="]);

    let opening = codes(&["/a.txt", "/b.txt"], &["file:///a.txt", "file:///b.txt"]);
    assert_eq!(
        Exec::expand("viewer %F", &opening),
        ["viewer", "/a.txt", "/b.txt"]
    );
    assert_eq!(
        Exec::expand("browser %U", &opening),
        ["browser", "file:///a.txt", "file:///b.txt"]
    );
    assert_eq!(Exec::expand("editor %f", &opening), ["editor", "/a.txt"]);
}

#[test]
fn command_like_name_stays_one_argument() {
    let opening = codes(&["/tmp/; rm -rf x"], &[]);
    assert_eq!(
        Exec::expand("editor %f", &opening),
        ["editor", "/tmp/; rm -rf x"]
    );
}

#[test]
fn other_field_codes_expand() {
    assert_eq!(argv("app %c"), ["app", "Test Application"]);
    assert_eq!(argv("app %i"), ["app", "--icon", "test-icon"]);
    assert_eq!(argv("app %k"), ["app", "/x/test.desktop"]);
    assert_eq!(argv("app %%s"), ["app", "%s"]);
    assert_eq!(argv("app %d %n"), ["app"], "deprecated codes are dropped");
    assert_eq!(
        argv("printf %s"),
        ["printf", "%s"],
        "and a stray one is kept"
    );
}
