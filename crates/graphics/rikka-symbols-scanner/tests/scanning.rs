use std::process::Command;

use rikka_test_fonts::font_path;

const SCANNER: &str = env!("CARGO_BIN_EXE_rikka-symbols-scanner");

const FONT: &str = "InterVariable.ttf";

struct Case {
    kind: &'static str,
    listing: &'static str,
    generated: Result<&'static str, &'static str>,
}

const CASES: &[Case] = &[
    Case {
        kind: "material",
        listing: "a 61\n7up 37\n",
        generated: Ok(concat!(
            "use super::MaterialSymbol;\n\n",
            "impl MaterialSymbol {\n",
            "    pub const A: Self = Self('\\u{61}');\n",
            "    pub const _7UP: Self = Self('\\u{37}');\n",
            "}\n\n",
            "pub(super) static NAMED: &[(&str, MaterialSymbol)] = &[\n",
            "    (\"7up\", MaterialSymbol('\\u{37}')),\n",
            "    (\"a\", MaterialSymbol('\\u{61}')),\n",
            "];\n",
        )),
    },
    Case {
        kind: "brands",
        listing: r#"{
            "a": {"unicode": "61", "styles": ["brands"], "aliases": {"names": ["letter-a"]}},
            "b": {"unicode": "62", "styles": ["solid"]}
        }"#,
        generated: Ok(concat!(
            "use super::BrandSymbol;\n\n",
            "impl BrandSymbol {\n",
            "    pub const A: Self = Self('\\u{61}');\n",
            "    pub const LETTER_A: Self = Self('\\u{61}');\n",
            "}\n\n",
            "pub(super) static NAMED: &[(&str, BrandSymbol)] = &[\n",
            "    (\"a\", BrandSymbol('\\u{61}')),\n",
            "    (\"letter-a\", BrandSymbol('\\u{61}')),\n",
            "];\n",
        )),
    },
    Case {
        kind: "material",
        listing: "a 61\nclose e5cd\n",
        generated: Err("close (U+E5CD)"),
    },
    Case {
        kind: "material",
        listing: "a 61\nA 62\n",
        generated: Err("both map to constant A"),
    },
];

#[test]
fn tables_list_exactly_covered_icons() {
    for case in CASES {
        let directory = tempfile::tempdir().unwrap();
        let listing = directory.path().join("listing");
        let output = directory.path().join("names.rs");
        std::fs::write(&listing, case.listing).unwrap();
        let result = Command::new(SCANNER)
            .arg(case.kind)
            .arg(&listing)
            .arg(font_path(FONT))
            .arg(&output)
            .output()
            .unwrap();
        let generated = std::fs::read_to_string(&output).ok();
        let stderr = String::from_utf8_lossy(&result.stderr);
        match case.generated {
            Ok(expected) => {
                assert!(result.status.success(), "{}: {stderr}", case.listing);
                assert_eq!(generated.as_deref(), Some(expected));
            }
            Err(named) => {
                assert!(!result.status.success(), "{} was accepted", case.listing);
                assert!(stderr.contains(named), "{stderr} does not name {named}");
                assert_eq!(generated, None);
            }
        }
    }
}
