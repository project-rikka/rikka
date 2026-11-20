use std::sync::Arc;

use rikka_proton_account::Account;
use rikka_proton_api::base64;
use rikka_proton_calendar::CalendarClient;
use rikka_proton_mail::MailClient;
use rikka_proton_pgp::{Pgp, PrivateKey, SessionKey};
use rikka_test_proton::{
    ADDRESS_ID, EMAIL, Owner, PASSPHRASE, Request, ResponseFuture, lock_key, new_key, signed_in_api,
};
use serde_json::{Value, json};

pub(crate) const CALENDAR_PASSPHRASE: &str = "calendar-passphrase-value";

pub(crate) fn decoded_query(request: &Request) -> String {
    request
        .query
        .replace("%5B%5D", "[]")
        .replace("%2F", "/")
        .replace("%40", "@")
}

pub(crate) struct CalendarFixture {
    pub(crate) key: PrivateKey,
    pub(crate) old_key: PrivateKey,
    pub(crate) bootstrap: Value,
    pub(crate) passphrases: Value,
}

pub(crate) fn calendar_fixture(pgp: &Pgp, address_key: &PrivateKey) -> CalendarFixture {
    let key = new_key(pgp, "");
    let old_key = new_key(pgp, "");
    let member_passphrase = |passphrase: &str| -> Value {
        let session = SessionKey::generate();
        let message = pgp
            .encrypt()
            .to(address_key.public_key())
            .with_session_key(session)
            .as_utf8()
            .armored_message(passphrase)
            .unwrap();
        let signature = pgp
            .sign()
            .with_key(address_key.clone())
            .as_utf8()
            .armored_detached(passphrase)
            .unwrap();
        json!({"MemberID": "member", "Passphrase": message, "Signature": signature})
    };
    let current = json!({"ID": "current", "Flags": 1, "MemberPassphrases": [member_passphrase(CALENDAR_PASSPHRASE)]});
    let old = json!({"ID": "old", "Flags": 0, "MemberPassphrases": [member_passphrase("old-passphrase")]});
    let bootstrap = json!({
        "Code": 1000,
        "Keys": [
            {"ID": "key", "CalendarID": "calendar", "PassphraseID": "current", "Flags": 3, "PrivateKey": lock_key(&key, CALENDAR_PASSPHRASE)},
            {"ID": "old-key", "CalendarID": "calendar", "PassphraseID": "old", "Flags": 1, "PrivateKey": lock_key(&old_key, "old-passphrase")}
        ],
        "Passphrase": current,
        "Members": [{
            "ID": "member",
            "CalendarID": "calendar",
            "AddressID": ADDRESS_ID,
            "Email": EMAIL,
            "Permissions": 127,
            "Flags": 1,
            "Name": "Personal",
            "Color": "#000000",
            "Display": 1
        }],
        "CalendarSettings": {
            "ID": "settings",
            "CalendarID": "calendar",
            "DefaultEventDuration": 30,
            "DefaultPartDayNotifications": [{"Type": 1, "Trigger": "-PT15M"}],
            "DefaultFullDayNotifications": [{"Type": 1, "Trigger": "-PT15H"}]
        }
    });
    CalendarFixture {
        key,
        old_key,
        bootstrap,
        passphrases: json!({"Code": 1000, "Passphrases": [current, old]}),
    }
}

pub(crate) fn base64_bytes(text: &str) -> Vec<u8> {
    base64::decode(text).unwrap()
}

pub(crate) struct SignedIn {
    pub(crate) client: CalendarClient,
    pub(crate) mail: MailClient,
}

pub(crate) async fn signed_in<H>(address_key: PrivateKey, handler: H) -> SignedIn
where
    H: Fn(Request) -> ResponseFuture + Send + Sync + 'static,
{
    let owner = Owner::with_address(address_key);
    let account = Account::new(
        signed_in_api(move |request: Request| {
            owner.routes(&request).unwrap_or_else(|| handler(request))
        })
        .await,
    );
    let keys = Arc::new(account.unlock_with_passphrase(PASSPHRASE).await.unwrap());

    SignedIn {
        client: CalendarClient::for_account(&account, &keys),
        mail: MailClient::for_account(&account, &keys),
    }
}
