use std::path::{Path, PathBuf};
use std::process::Command;

const GENERATOR: &str = env!("CARGO_BIN_EXE_rikka-text-unicode-gen");
const CRATE: &str = "crates/text/rikka-text-unicode";

macro_rules! embedded {
    ($directory:literal: $($name:literal),* $(,)?) => {
        [$((
            concat!($directory, "/", $name),
            include_str!(concat!("../../rikka-text-unicode/", $directory, "/", $name)),
        )),*]
    };
}

macro_rules! committed {
    ($($name:literal),* $(,)?) => {
        [$(($name, &include_bytes!(concat!("../../rikka-text-unicode/src/tables/", $name))[..])),*]
    };
}

fn scratch_root() -> PathBuf {
    let base = std::env::var_os("TEST_TMPDIR").map_or_else(std::env::temp_dir, PathBuf::from);
    let root = base.join(format!("text-unicode-gen-{}", std::process::id()));
    std::fs::create_dir_all(&root).expect("failed to create scratch root");
    root
}

fn vendor(root: &Path, sources: &[(&str, &str)]) {
    for (name, contents) in sources {
        let path = root.join(CRATE).join(name);
        std::fs::create_dir_all(path.parent().expect("source path has no parent"))
            .expect("failed to create source dir");
        std::fs::write(path, contents).expect("failed to write source");
    }
}

#[test]
fn committed_tables_are_fresh() {
    let root = scratch_root();
    vendor(
        &root,
        &embedded!["ucd":
            "BidiBrackets.txt",
            "BidiMirroring.txt",
            "CompositionExclusions.txt",
            "DerivedCoreProperties.txt",
            "EastAsianWidth.txt",
            "IndicPositionalCategory.txt",
            "IndicSyllabicCategory.txt",
            "LineBreak.txt",
            "PropertyValueAliases.txt",
            "ScriptExtensions.txt",
            "Scripts.txt",
            "UnicodeData.txt",
            "auxiliary/GraphemeBreakProperty.txt",
            "auxiliary/WordBreakProperty.txt",
            "emoji/emoji-data.txt",
            "extracted/DerivedBidiClass.txt",
            "extracted/DerivedJoiningGroup.txt",
            "extracted/DerivedJoiningType.txt",
        ],
    );
    vendor(
        &root,
        &embedded!["dictionaries":
            "thaidict.txt",
            "laodict.txt",
            "khmerdict.txt",
            "burmesedict.txt",
            "cjdict.txt",
        ],
    );
    let committed = committed![
        "mod.rs",
        "enums.rs",
        "scripts.rs",
        "properties.rs",
        "shaping.rs",
        "normalization.rs",
        "bidi.rs",
        "line_break.rs",
        "dictionaries.rs",
        "thai.dict",
        "lao.dict",
        "khmer.dict",
        "burmese.dict",
        "cj.dict",
    ];

    let output = Command::new(GENERATOR)
        .arg(&root)
        .output()
        .expect("failed to run generator");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let tables = root.join(CRATE).join("src/tables");
    let mut generated: Vec<String> = std::fs::read_dir(&tables)
        .expect("failed to list generated tables")
        .map(|entry| {
            entry
                .expect("failed to read table entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    generated.sort_unstable();
    let mut expected: Vec<&str> = committed.iter().map(|(name, _)| *name).collect();
    expected.sort_unstable();
    assert_eq!(generated, expected);
    for (name, contents) in committed {
        assert!(
            std::fs::read(tables.join(name)).expect("failed to read generated table") == contents,
            "src/tables/{name} is stale: run bazel run //crates/text/rikka-text-unicode-gen -- \"$PWD\""
        );
    }
}
