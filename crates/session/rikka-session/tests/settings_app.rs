use rikka_session::settings_app::{Link, PROGRAM, Section, Spot};

#[test]
fn every_link_round_trips_through_its_argv() {
    let links = Section::ALL
        .into_iter()
        .map(Link::from)
        .chain(Spot::ALL.into_iter().map(Link::from));
    for link in links {
        let argv = link.argv();
        assert_eq!(argv.first(), Some(&PROGRAM), "{link:?}");
        assert_eq!(Link::from_arguments(&argv), Some(link), "{argv:?}");
    }
}

#[test]
fn arguments_resolve_to_the_named_place() {
    let cases: [(&[&str], Option<Link>); 6] = [
        (
            &[
                "/run/current-system/sw/bin/rikka-settings",
                "appearance",
                "wallpaper",
            ],
            Some(Link::Spot(Spot::Wallpaper)),
        ),
        (&["audio", "input"], Some(Link::Spot(Spot::SoundInput))),
        (&["audio", "wallpaper"], Some(Link::Section(Section::Audio))),
        (
            &["network", "--verbose"],
            Some(Link::Section(Section::Network)),
        ),
        (&["rikka-settings"], None),
        (&["wallpaper"], None),
    ];
    for (arguments, expected) in cases {
        assert_eq!(Link::from_arguments(arguments), expected, "{arguments:?}");
    }
}
