use browser_store::{ProfileId, SpaceFields, SpaceIcon, SpaceId, Store};

pub(crate) fn space(store: &Store, name: &str) -> (ProfileId, SpaceId) {
    let profile = store.create_profile("Default");
    let space = store.create_space(SpaceFields {
        name: name.to_owned(),
        icon: SpaceIcon::Emoji("🌊".to_owned()),
        profile,
    });
    (profile, space)
}
