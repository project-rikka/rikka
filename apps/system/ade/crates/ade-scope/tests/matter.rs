use ade_scope::matter::read;
use ade_scope::{Fields, FrontMatterError, List, Text};
use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
struct Front {
    #[serde(default)]
    name: Text,
    #[serde(default)]
    description: Text,
    #[serde(default)]
    generated: Fields,
}

#[test]
fn splits_front_matter_from_body() {
    let (front, body): (Front, &str) =
        read("---\nname: pdf\ndescription: Extracts text from PDFs.\n---\n\n# PDF\n")
            .expect("the document is well formed");

    assert_eq!(front.name.some(), Some("pdf"));
    assert_eq!(front.description.some(), Some("Extracts text from PDFs."));
    assert_eq!(body, "# PDF");
}

#[test]
fn repairs_unquoted_colon() {
    let (front, body): (Front, &str) =
        read("---\nname: review\ndescription: Use it when: somebody asks\n---\nBody\n")
            .expect("it is mended and read");

    assert_eq!(front.description.some(), Some("Use it when: somebody asks"));
    assert_eq!(body, "Body");
}

#[test]
fn non_text_value_is_ignored() {
    let (front, _): (Front, &str) = read(
        "---\nname:\n  - a\n  - b\ndescription: Still here.\ngenerated: nothing-shaped\n---\nB\n",
    )
    .expect("the shapes it does not expect are not a reason to refuse it");

    assert!(front.name.some().is_none());
    assert_eq!(front.description.some(), Some("Still here."));
    assert!(front.generated.into_map().is_empty());
}

#[test]
fn fence_in_body_keeps_front_matter() {
    let (_, body): (Front, &str) =
        read("---\nname: a\ndescription: b\n---\nwrite --- here\n").expect("well formed");

    assert_eq!(body, "write --- here");
}

#[test]
fn rejects_missing_or_unclosed_front_matter() {
    assert!(matches!(
        read::<Front>("# Just a heading\n"),
        Err(FrontMatterError::Missing)
    ));
    assert!(matches!(
        read::<Front>("---\nname: a\n\nNo fence.\n"),
        Err(FrontMatterError::Unclosed)
    ));
}

#[derive(Debug, Default, Deserialize)]
struct Written {
    #[serde(default)]
    name: Text,
    #[serde(default)]
    description: Text,
    #[serde(default)]
    license: Text,
    #[serde(default)]
    metadata: Fields,
    #[serde(default)]
    tags: List,
    #[serde(default, rename = "allowed-tools")]
    allowed_tools: List,
}

#[test]
fn reads_shapes_skills_are_written_in() {
    let document = r#"---
# a skill somebody else wrote
name: "pdf-\u0074ools"   # quoted, with an escape
description: >-
  Extracts text and tables from PDFs,
  fills in forms.

  Use it when: a PDF is mentioned.
license: 'Apache-2.0, as the author''s LICENSE says'
metadata:
  author: example-org   # who
  version: "1.0"
  nested:
    deeper: ignored
tags: [documents, "pdf, forms", 'text']
allowed-tools:
  - Bash(pdftotext:*)
  - Read
unknown: ~
---
Body
"#;
    let (front, body): (Written, &str) = read(document).expect("the document is well formed");

    assert_eq!(front.name.some(), Some("pdf-tools"));
    assert_eq!(
        front.description.some(),
        Some(
            "Extracts text and tables from PDFs, fills in forms.\nUse it when: a PDF is mentioned."
        )
    );
    assert_eq!(
        front.license.some(),
        Some("Apache-2.0, as the author's LICENSE says")
    );
    let metadata = front.metadata.into_map();
    assert_eq!(
        metadata.get("author").map(String::as_str),
        Some("example-org")
    );
    assert_eq!(metadata.get("version").map(String::as_str), Some("1.0"));
    assert_eq!(
        front.tags.into_vec(),
        ["documents", "pdf, forms", "text"].map(str::to_owned)
    );
    assert_eq!(
        front.allowed_tools.into_vec(),
        ["Bash(pdftotext:*)", "Read"].map(str::to_owned)
    );
    assert_eq!(body, "Body");

    let (literal, _): (Written, &str) =
        read("---\ndescription: |\n  first line\n    kept indented\nname: after\n---\n")
            .expect("a literal block is read");
    assert_eq!(
        literal.description.some(),
        Some("first line\n  kept indented")
    );
    assert_eq!(literal.name.some(), Some("after"));

    let (wrapped, _): (Written, &str) =
        read("---\ndescription: one sentence\n  carried onto the next line\n---\n")
            .expect("a wrapped plain value is read");
    assert_eq!(
        wrapped.description.some(),
        Some("one sentence carried onto the next line")
    );
}
