use browser_model::{Ask, Grants, Plan, Requester, Scope, WindowId, plan};
use browser_store::{Decision, ProfileId};

#[test]
fn plans_follow_every_remembered_decision() {
    let asks = || Some(vec![Ask::Camera, Ask::Microphone]);
    let cases = [
        (None, None, Plan::Prompt(vec![Ask::Camera, Ask::Microphone])),
        (
            Some(Decision::Allow),
            None,
            Plan::Prompt(vec![Ask::Camera, Ask::Microphone]),
        ),
        (Some(Decision::Allow), Some(Decision::Allow), Plan::Allow),
        (Some(Decision::Allow), Some(Decision::Deny), Plan::Deny),
    ];
    for (camera, microphone, expected) in cases {
        let decided = |ask: Ask| match ask {
            Ask::Camera => camera,
            Ask::Microphone => microphone,
            _ => None,
        };
        assert_eq!(plan(asks(), decided), expected, "{camera:?} {microphone:?}");
    }
    assert_eq!(plan(None, |_| Some(Decision::Allow)), Plan::Deny);
}

#[test]
fn grants_are_kept_per_scope_and_forgotten_per_host() {
    let mut grants = Grants::default();
    let profile = Scope::Profile(ProfileId::from_raw(1));
    let private = Scope::Window(WindowId(1));
    let site = Requester::origin("https://maps.example.com");
    let other = Requester::origin("https://other.example.org");

    assert!(grants.remember(profile, &site, Ask::Location, Some(Decision::Allow)));
    assert!(!grants.remember(profile, &site, Ask::Location, Some(Decision::Allow)));
    assert!(grants.remember(profile, &other, Ask::Camera, Some(Decision::Deny)));
    assert!(!grants.remember(profile, &site, Ask::Screen, Some(Decision::Allow)));
    assert_eq!(
        grants.decision(profile, &site, Ask::Location),
        Some(Decision::Allow)
    );
    assert_eq!(grants.decision(private, &site, Ask::Location), None);

    let forgotten = grants.forget_site(profile, "maps.example.com");
    assert_eq!(forgotten, [(site.clone(), Ask::Location)]);
    assert_eq!(grants.decision(profile, &site, Ask::Location), None);
    assert_eq!(
        grants.decision(profile, &other, Ask::Camera),
        Some(Decision::Deny)
    );
}
