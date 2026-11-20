use rikka_certificate_transparency::{Error, LogList, LogListKey};
use rikka_test_certificate_transparency::{CHROME_LIST, CHROME_SIGNATURE, Signer, log_list_at};

#[test]
fn chrome_signature_verifies_against_pinned_key() {
    let list = LogList::from_signed(CHROME_LIST, CHROME_SIGNATURE, &LogListKey::chrome()).unwrap();

    assert!(list.logs().count() > 0);
}

#[test]
fn rejects_altered_or_foreign_signatures() {
    let signer = Signer::new();
    let list = log_list_at("2030-01-01T00:00:00Z");
    let signature = signer.sign(&list);
    assert!(LogList::from_signed(&list, &signature, &signer.key()).is_ok());

    let refused = [
        (log_list_at("2030-01-02T00:00:00Z"), signer.key()),
        (list.clone(), Signer::new().key()),
        (list, LogListKey::chrome()),
    ];
    for (list, key) in refused {
        assert!(matches!(
            LogList::from_signed(&list, &signature, &key),
            Err(Error::Signature)
        ));
    }
}
