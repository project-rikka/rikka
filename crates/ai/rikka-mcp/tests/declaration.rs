use rikka_mcp::{Http, Server, Stdio};

#[test]
fn command_line_splits_into_arguments() {
    let stdio = Stdio::parse("npx -y '@scope/server' ~/src").expect("quotes should close");

    assert_eq!(stdio.command(), "npx");
    assert_eq!(stdio.arguments(), ["-y", "@scope/server", "~/src"]);
    assert_eq!(stdio.command_line(), "npx -y @scope/server ~/src");
    assert!(Stdio::parse("npx 'open").is_err());
}

#[test]
fn unusable_declaration_fails_early() {
    assert!(
        Server::new("github", Stdio::new("npx").arg("-y"))
            .config_error()
            .is_none()
    );
    assert!(Server::new("", Stdio::new("npx")).config_error().is_some());
    assert!(
        Server::new("github", Stdio::new(" "))
            .config_error()
            .is_some()
    );
    assert!(
        Server::new("linear", Http::new("ftp://example.invalid"))
            .config_error()
            .is_some()
    );
    assert!(
        Server::new("linear", Http::new("https://mcp.example.invalid/mcp"))
            .config_error()
            .is_none()
    );
}
