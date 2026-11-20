use shell_kit::Presence;

#[test]
fn every_surface_sharing_a_presence_sees_its_change() {
    let presence = Presence::default();
    let frost = presence.clone();
    let overview = presence.clone();

    presence.set(true);
    assert!(frost.take_moved());
    assert!(overview.take_moved());
    assert!(!frost.take_moved());

    presence.set(false);
    assert!(overview.take_moved());
    assert!(frost.take_moved());
}
