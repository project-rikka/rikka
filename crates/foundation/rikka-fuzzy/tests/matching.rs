use rikka_fuzzy::{Haystack, Needle, best, byte_ranges};

fn positions(needle: &Needle, text: &str) -> Vec<usize> {
    let mut positions = Vec::new();
    needle
        .positions(&Haystack::new(text), &mut positions)
        .expect("needle matches");
    positions
}

#[test]
fn subsequence_matches_and_strangers_do_not() {
    let needle = Needle::new("fox");

    assert!(needle.score("Firefox").is_some());
    assert!(needle.score("fx").is_none());
}

#[test]
fn word_starts_and_runs_outrank_scattered_letters() {
    let needle = Needle::new("set");
    let ranked = best(
        ["closest", "rikka-settings", "s/e/t", "soft reset"]
            .into_iter()
            .filter_map(|candidate| Some((needle.score(candidate)?, candidate))),
        2,
    );

    let names: Vec<&str> = ranked.into_iter().map(|(_, name)| name).collect();
    assert_eq!(names, ["rikka-settings", "s/e/t"]);
}

#[test]
fn file_name_outranks_folder_name() {
    let needle = Needle::new("main");

    assert!(needle.score("src/main.rs") > needle.score("maintenance/notes.txt"));
    assert!(needle.score("src/main.rs") > needle.score("src/domain.rs"));
}

#[test]
fn positions_follow_the_scored_alignment() {
    let needle = Needle::new("fb");

    assert_eq!(positions(&needle, "foo_bar"), [0, 4]);
    assert_eq!(positions(&needle, "fab/FooBar"), [4, 7]);
    assert_eq!(byte_ranges("naïve_ß", &[2, 3, 6]), [2..5, 7..9]);
}

#[test]
fn every_atom_must_match_in_any_order() {
    let needle = Needle::new("code visual");

    assert_eq!(
        positions(&needle, "Visual Studio Code"),
        [0, 1, 2, 3, 4, 5, 14, 15, 16, 17]
    );
    assert!(needle.score("Visual Studio").is_none());
}

#[test]
fn smart_case_is_exact_only_for_capitalised_atoms() {
    assert!(
        Needle::with_smart_case("readme")
            .score("README.md")
            .is_some()
    );
    assert!(
        Needle::with_smart_case("Readme")
            .score("README.md")
            .is_none()
    );
    assert!(Needle::new("Readme").score("README.md").is_some());
}

#[test]
fn long_haystacks_still_match_and_mark() {
    let long = format!("{}needle", "x".repeat(400));
    let needle = Needle::new("needle");

    assert!(needle.score(&long).is_some());
    assert_eq!(positions(&needle, &long), [400, 401, 402, 403, 404, 405]);
}
