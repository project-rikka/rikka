use std::io::Write as _;
use std::process::{Command, Stdio};

pub fn assert_formatted(source: &str) {
    let mut nixfmt = Command::new("nixfmt")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("nixfmt is on PATH");
    nixfmt
        .stdin
        .take()
        .expect("nixfmt reads its input")
        .write_all(source.as_bytes())
        .expect("nixfmt takes the source");
    let formatted = nixfmt.wait_with_output().expect("nixfmt finishes");
    assert!(
        formatted.status.success(),
        "nixfmt refused it: {}\n{source}",
        String::from_utf8_lossy(&formatted.stderr)
    );
    let formatted = String::from_utf8(formatted.stdout).expect("nixfmt writes UTF-8");
    assert_eq!(formatted, source, "nixfmt would rewrite it");
}
