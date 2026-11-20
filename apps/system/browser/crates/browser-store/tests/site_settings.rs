use browser_store::{Decision, Permission, Store};

#[test]
fn permissions_are_kept_per_origin_and_kind() {
    let store = Store::in_memory().unwrap();
    let profile = store.create_profile("Default");
    store.set_permission(
        profile,
        "https://Meet.example",
        Permission::Camera,
        Decision::Allow,
    );
    store.set_permission(
        profile,
        "https://meet.example",
        Permission::Microphone,
        Decision::Deny,
    );
    store.set_permission(
        profile,
        "http://meet.example:8080",
        Permission::Camera,
        Decision::Deny,
    );
    store.set_permission(
        profile,
        "https://meet.example",
        Permission::Microphone,
        Decision::Allow,
    );
    store.flush().unwrap();

    assert_eq!(
        store
            .permission(profile, "https://meet.example", Permission::Camera)
            .unwrap(),
        Some(Decision::Allow)
    );
    assert_eq!(
        store
            .permission(profile, "https://meet.example", Permission::Microphone)
            .unwrap(),
        Some(Decision::Allow)
    );
    assert_eq!(
        store
            .permission(profile, "http://meet.example:8080", Permission::Camera)
            .unwrap(),
        Some(Decision::Deny)
    );
    assert_eq!(
        store
            .permission(profile, "https://meet.example", Permission::Geolocation)
            .unwrap(),
        None
    );

    store.clear_permission(profile, "https://meet.example", Permission::Camera);
    store.flush().unwrap();
    assert_eq!(
        store
            .permission(profile, "https://meet.example", Permission::Camera)
            .unwrap(),
        None
    );
}

#[test]
fn storage_access_keyed_by_origin_and_site() {
    let store = Store::in_memory().unwrap();
    let profile = store.create_profile("Default");
    let embed = "https://embed.example";
    store.set_storage_access(profile, embed, "https://news.example", Decision::Allow);
    store.set_storage_access(profile, embed, "https://shop.example", Decision::Deny);
    store.flush().unwrap();

    assert_eq!(
        store
            .storage_access(profile, embed, "https://news.example")
            .unwrap(),
        Some(Decision::Allow)
    );
    assert_eq!(
        store
            .storage_access(profile, embed, "https://shop.example")
            .unwrap(),
        Some(Decision::Deny)
    );
    assert_eq!(
        store
            .storage_access(profile, embed, "https://other.example")
            .unwrap(),
        None
    );
    assert_eq!(
        store
            .permission(profile, embed, Permission::StorageAccess)
            .unwrap(),
        None
    );

    let listed = store
        .site_settings(profile, "embed.example")
        .unwrap()
        .permissions;
    let tops: Vec<_> = listed
        .iter()
        .map(|entry| entry.top_site.as_deref())
        .collect();
    assert_eq!(
        tops,
        [Some("https://news.example"), Some("https://shop.example")]
    );

    store.set_permission(profile, embed, Permission::StorageAccess, Decision::Deny);
    store.clear_storage_access(profile, embed, "https://news.example");
    store.flush().unwrap();
    assert_eq!(
        store
            .storage_access(profile, embed, "https://news.example")
            .unwrap(),
        Some(Decision::Deny)
    );
    assert_eq!(
        store
            .storage_access(profile, embed, "https://shop.example")
            .unwrap(),
        Some(Decision::Deny)
    );

    store.clear_site(profile, "embed.example");
    store.flush().unwrap();
    assert_eq!(
        store
            .storage_access(profile, embed, "https://shop.example")
            .unwrap(),
        None
    );
}
