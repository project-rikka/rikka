use std::io::Write;
use std::path::PathBuf;

use nixpkgs_catalog::{Catalog, Error, Id, License, Package};
use rikka_types::CancelToken;

const INDEX: &[u8] = include_bytes!("packages.json");
const RELEASE: &str = "nixos-26.11pre1090327.7fd4004d0511";

struct Scratch {
    folder: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Self {
        let folder = std::env::temp_dir().join(format!(
            "nixpkgs-search-catalog-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).expect("a scratch folder");
        Self { folder }
    }

    fn write_index(&self, index: &[u8]) -> PathBuf {
        let path = self.folder.join("packages.json.br");
        let file = std::fs::File::create(&path).expect("a compressed index");
        let mut writer = brotli::CompressorWriter::new(file, 4096, 5, 22);
        writer.write_all(index).expect("compressed");
        writer.flush().expect("flushed");
        path
    }

    fn catalog_path(&self) -> PathBuf {
        self.folder.join("7fd4004d0511.catalog")
    }

    fn import(&self) -> Catalog {
        Catalog::import(
            &self.write_index(INDEX),
            RELEASE,
            &self.catalog_path(),
            &CancelToken::new(),
        )
        .expect("the channel index imports")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.folder);
    }
}

fn attributes(catalog: &Catalog, ids: &[Id]) -> Vec<String> {
    ids.iter()
        .map(|id| catalog.entry(*id).attribute.to_owned())
        .collect()
}

fn package(catalog: &Catalog, attribute: &str) -> Package {
    let id = catalog
        .find(attribute)
        .unwrap_or_else(|| panic!("{attribute} is listed"));
    catalog.package(id).expect("its details read")
}

fn labels(package: &Package) -> Vec<&str> {
    package.licenses.iter().map(License::label).collect()
}

#[test]
fn lists_every_package_by_attribute() {
    let scratch = Scratch::new("listed");
    let catalog = scratch.import();

    let all: Vec<Id> = catalog.ids().collect();
    assert_eq!(
        attributes(&catalog, &all),
        [
            "aroccStdenv",
            "chickenPackages_5.chickenEggs.pebble",
            "discord",
            "firefox",
            "hello",
            "httpx",
            "python313Packages.httpx",
            "ripgrep",
            "ripgrep-all",
        ]
    );
    assert_eq!(catalog.release(), RELEASE);

    let ripgrep = catalog.entry(catalog.find("ripgrep").expect("ripgrep is listed"));
    assert_eq!(ripgrep.name, "ripgrep");
    assert_eq!(ripgrep.version, "15.2.0");
    assert!(ripgrep.description.contains("Silver Searcher"));
    assert_eq!(ripgrep.set(), None);
    let httpx = catalog.entry(catalog.find("python313Packages.httpx").expect("listed"));
    assert_eq!(httpx.set(), Some("python313Packages"));
    assert!(
        catalog
            .entry(catalog.find("discord").expect("listed"))
            .unfree
    );
    assert_eq!(catalog.find("aroccstdenv"), None);
    assert_eq!(catalog.find("missing"), None);
}

#[test]
fn details_match_nixpkgs_metadata() {
    let scratch = Scratch::new("details");
    let catalog = scratch.import();

    let ripgrep = package(&catalog, "ripgrep");
    assert_eq!(labels(&ripgrep), ["Unlicense", "MIT"]);
    assert_eq!(ripgrep.main_program.as_deref(), Some("rg"));
    assert_eq!(ripgrep.outputs, ["out"]);
    assert_eq!(ripgrep.homepages, ["https://github.com/BurntSushi/ripgrep"]);
    assert_eq!(
        ripgrep.position.as_deref(),
        Some("pkgs/by-name/ri/ripgrep/package.nix:66")
    );
    assert!(
        ripgrep
            .platforms
            .iter()
            .any(|platform| platform == "x86_64-linux")
    );
    assert!(ripgrep.maintainers.iter().all(|who| who.label().is_some()));
    assert!(!ripgrep.unfree);

    let pebble = package(&catalog, "chickenPackages_5.chickenEggs.pebble");
    assert_eq!(labels(&pebble), ["agpl-3.0-only"]);
    assert_eq!(pebble.homepages.len(), 2);
    assert_eq!(pebble.platforms, ["x86_64-linux"]);
    assert!(pebble.outputs.is_empty());
    assert_eq!(pebble.position, None);
    assert_eq!(pebble.set.as_deref(), Some("chickenPackages_5.chickenEggs"));

    assert_eq!(labels(&package(&catalog, "aroccStdenv")), ["mit", "0BSD"]);
    assert_eq!(
        package(&catalog, "python313Packages.httpx").outputs,
        ["dist", "out"]
    );
    assert!(package(&catalog, "discord").unfree);
}

#[test]
fn search_ranks_names_before_descriptions() {
    let scratch = Scratch::new("search");
    let catalog = scratch.import();

    let cases: [(&str, &[&str]); 6] = [
        ("httpx", &["httpx", "python313Packages.httpx"]),
        ("RipGrep", &["ripgrep", "ripgrep-all"]),
        ("rip all", &["ripgrep-all"]),
        ("web browser", &["firefox"]),
        ("pebble static", &["chickenPackages_5.chickenEggs.pebble"]),
        ("nothing-is-called-this", &[]),
    ];
    for (query, expected) in cases {
        let attributes_found = attributes(&catalog, &catalog.search(query));
        assert!(
            attributes_found.starts_with(
                &expected
                    .iter()
                    .map(|one| (*one).to_owned())
                    .collect::<Vec<_>>()
            ),
            "{query}: {attributes_found:?}"
        );
        if expected.is_empty() {
            assert!(attributes_found.is_empty(), "{query}: {attributes_found:?}");
        }
    }
    assert_eq!(catalog.search("  ").len(), catalog.len());
}

#[test]
fn cached_catalog_loads_without_index() {
    let scratch = Scratch::new("cached");
    let imported = scratch.import();
    let published = scratch.folder.join("packages.json.br");
    std::fs::remove_file(published).expect("the download is gone");

    let loaded = Catalog::load(&scratch.catalog_path()).expect("failed to load cached catalog");

    assert_eq!(loaded.release(), imported.release());
    assert_eq!(
        attributes(&loaded, &loaded.ids().collect::<Vec<_>>()),
        attributes(&imported, &imported.ids().collect::<Vec<_>>())
    );
    assert_eq!(package(&loaded, "firefox"), package(&imported, "firefox"));
}

#[test]
fn rejects_corrupt_catalog() {
    let scratch = Scratch::new("corrupt");
    drop(scratch.import());
    let catalog_path = scratch.catalog_path();
    let whole = std::fs::read(&catalog_path).expect("failed to read catalog");

    for corrupt in [&whole[..whole.len() / 2], &b"not a catalog"[..], &[][..]] {
        std::fs::write(&catalog_path, corrupt).expect("failed to write corrupt catalog");
        let opened = Catalog::load(&catalog_path);
        assert!(matches!(opened, Err(Error::Read { .. })), "{opened:?}");
    }
}

#[test]
fn rejects_foreign_index() {
    let scratch = Scratch::new("foreign");

    for foreign in [
        &b"{\"version\":\"2\"}"[..],
        &b"[1, 2]"[..],
        &b"{\"packages\":"[..],
    ] {
        let imported = Catalog::import(
            &scratch.write_index(foreign),
            RELEASE,
            &scratch.catalog_path(),
            &CancelToken::new(),
        );
        assert!(
            matches!(imported, Err(Error::Index(_) | Error::EmptyIndex)),
            "{imported:?}"
        );
        assert!(!scratch.catalog_path().exists());
    }
}

#[test]
fn cancelled_import_leaves_no_file() {
    let scratch = Scratch::new("stopped");
    let cancel = CancelToken::new();
    cancel.cancel();

    let imported = Catalog::import(
        &scratch.write_index(INDEX),
        RELEASE,
        &scratch.catalog_path(),
        &cancel,
    );

    assert!(matches!(imported, Err(Error::Cancelled)), "{imported:?}");
    assert!(!scratch.catalog_path().exists());
}
