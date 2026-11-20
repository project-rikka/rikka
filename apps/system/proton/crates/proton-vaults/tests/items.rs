use jiff::Timestamp;
use proton_vaults::{
    Card, CustomField, DecryptedItem, Draft, Entry, ItemType, OpenedItem, Ssh, Strength, Vault,
    Vaults, Wifi,
};
use rikka_proton_pass::{
    Item, ItemContent, ItemFlags, ItemId, ItemLogin, Metadata, Passkey, ShareId,
};

fn entry(share: &str, id: &str, title: &str, item_type: ItemType) -> Entry {
    Entry {
        share: ShareId::from(share),
        id: ItemId::from(id),
        item_type,
        title: title.to_owned(),
        subtitle: "person@proton.me".to_owned(),
        website: Some("https://proton.me".to_owned()),
        pinned: false,
        trashed: false,
        has_code: false,
        strength: Some(Strength::Strong),
        flags: ItemFlags::default(),
        revision: 1,
        created: Timestamp::from_second(5).unwrap(),
        modified: Timestamp::from_second(10).unwrap(),
        used: Timestamp::from_second(0).unwrap(),
    }
}

fn personal() -> Vault {
    Vault {
        share: ShareId::from("vault-1"),
        name: "Personal".to_owned(),
        description: String::new(),
        icon: 2,
        color: 2,
    }
}

fn unlocked(id: &str, title: &str, password: &str) -> DecryptedItem {
    DecryptedItem {
        entry: entry("vault-1", id, title, ItemType::Login),
        opened: OpenedItem {
            id: ItemId::from(id),
            title: title.to_owned(),
            password: password.to_owned(),
            ..opened(login(0))
        },
    }
}

fn opened(raw: Item) -> OpenedItem {
    OpenedItem {
        share: ShareId::from("vault-1"),
        id: ItemId::from("item-1"),
        revision: 3,
        item_type: ItemType::Login,
        title: "Proton".to_owned(),
        note: "the note".to_owned(),
        pinned: false,
        trashed: false,
        created: Timestamp::from_second(5).unwrap(),
        modified: Timestamp::from_second(10).unwrap(),
        used: Timestamp::from_second(0).unwrap(),
        email: "person@proton.me".to_owned(),
        username: "person".to_owned(),
        password: "secret".to_owned(),
        strength: Some(Strength::Strong),
        urls: vec!["https://proton.me".to_owned()],
        totp: String::new(),
        passkeys: 1,
        alias: String::new(),
        card: Card::default(),
        wifi: Wifi::default(),
        ssh: Ssh::default(),
        fields: Vec::new(),
        sections: Vec::new(),
        raw,
    }
}

fn login(passkeys: usize) -> Item {
    Item::new(
        Metadata {
            name: "Proton".to_owned(),
            note: "the note".to_owned(),
            item_uuid: "uuid-1".to_owned(),
            custom_icon: None,
        },
        ItemContent::Login(ItemLogin {
            item_email: "person@proton.me".to_owned(),
            item_username: "person".to_owned(),
            password: "secret".to_owned(),
            urls: vec!["https://proton.me".to_owned()],
            totp_uri: String::new(),
            passkeys: vec![Passkey::default(); passkeys],
            autofill_urls: Vec::new(),
        }),
    )
}

#[test]
fn builds_login_from_draft() {
    let mut draft = Draft::fresh(ShareId::from("vault-1"), ItemType::Login);
    draft.title = "  Proton  ".to_owned();
    draft.username = "person".to_owned();
    draft.password = "secret".to_owned();
    draft.urls = "https://proton.me\n\n https://account.proton.me ".to_owned();
    draft.fields = vec![
        CustomField::new("Recovery", "kept", true),
        CustomField::new("", "", false),
    ];

    let item = draft.item();
    let login = item.login().expect("item is not a login");

    assert_eq!(item.name(), "Proton");
    assert_eq!(login.item_username, "person");
    assert_eq!(
        login.urls,
        vec![
            "https://proton.me".to_owned(),
            "https://account.proton.me".to_owned()
        ]
    );
    assert_eq!(item.extra_fields.len(), 1);
    assert!(item.field("Recovery").expect("field missing").is_hidden());
}

#[test]
fn rewrite_preserves_unknown_fields() {
    let mut draft = Draft::from_opened(&opened(login(2)));
    draft.password = "another".to_owned();

    let item = draft.item();
    let login = item.login().expect("item is not a login");

    assert_eq!(login.password, "another");
    assert_eq!(login.passkeys.len(), 2);
    assert_eq!(item.uuid(), Some("uuid-1"));
}

#[test]
fn resync_replaces_vault_items() {
    let share = ShareId::from("vault-1");
    let mut vaults = Vaults::new();
    vaults.set_vaults(vec![personal()]);
    vaults.synced(
        &share,
        vec![
            unlocked("item-1", "Proton", "hunter2"),
            unlocked("item-2", "Bank", "hunter3"),
        ],
    );

    vaults.synced(
        &share,
        vec![
            unlocked("item-1", "Proton", "hunter4"),
            unlocked("item-3", "Shop", "hunter5"),
        ],
    );

    assert_eq!(vaults.all(), 2);
    assert_eq!(
        vaults
            .opened(&share, &ItemId::from("item-1"))
            .expect("changed item missing")
            .password,
        "hunter4"
    );
    assert!(vaults.opened(&share, &ItemId::from("item-2")).is_none());
    assert!(vaults.entry(&share, &ItemId::from("item-3")).is_some());
}

#[test]
fn debug_output_hides_secrets() {
    let share = ShareId::from("vault-1");
    let unlocked = unlocked("item-1", "Proton", "hunter2");
    let mut vaults = Vaults::new();
    vaults.set_vaults(vec![personal()]);
    vaults.synced(&share, vec![unlocked.clone()]);

    let printed = [
        format!("{vaults:?}"),
        format!("{unlocked:?}"),
        format!("{:?}", unlocked.opened),
        format!("{:?}", Draft::from_opened(&unlocked.opened)),
    ];

    for output in printed {
        assert!(!output.contains("hunter2"), "{output}");
        assert!(!output.contains("person@proton.me"), "{output}");
    }
}
