use google_passwords::{Recipe, posted_vault_keys};
use serde_json::json;

#[test]
fn posted_vault_keys_belong_to_the_account() {
    let body = json!({ "gaia": "1234", "keys": ["a2V5IG9uZQ==", "", "not base64!"] });

    assert_eq!(
        posted_vault_keys(&body, "1234"),
        Some(vec![b"key one".to_vec()])
    );
    assert_eq!(posted_vault_keys(&body, "5678"), None);
    assert_eq!(
        posted_vault_keys(&json!({ "gaia": "1234", "keys": [] }), "1234"),
        None
    );
    assert_eq!(posted_vault_keys(&json!("keys"), "1234"), None);
}

#[test]
fn generated_passwords_hold_every_asked_class() {
    let recipes = [
        Recipe::default(),
        Recipe {
            length: 3,
            digits: false,
            symbols: false,
        },
        Recipe {
            length: 500,
            digits: true,
            symbols: false,
        },
    ];
    for recipe in recipes {
        let password = recipe.generate();
        let has = |class: fn(&char) -> bool| password.chars().any(|letter| class(&letter));
        let length = recipe.length.clamp(Recipe::SHORTEST, Recipe::LONGEST);
        assert_eq!(password.chars().count(), length);
        assert!(has(char::is_ascii_lowercase) && has(char::is_ascii_uppercase));
        assert_eq!(has(char::is_ascii_digit), recipe.digits);
        assert_eq!(has(char::is_ascii_punctuation), recipe.symbols);
        assert_ne!(password, recipe.generate());
    }
}
