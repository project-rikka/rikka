use rikka_proton_pgp::{Pgp, PrivateKey, PublicKey};
use serde_json::{Value, json};

use crate::request::Request;
use crate::response::{ResponseFuture, json_response};

pub const PASSPHRASE: &[u8] = b"the mailbox passphrase";
pub const EMAIL: &str = "tester@proton.test";
pub const ADDRESS_ID: &str = "address-value";
const ADDRESS_KEY_ID: &str = "address-key";
const TOKEN: &str = "3d2b0d1f4c5a6e7b8c9d0e1f2a3b4c5d6e7f80917a2b3c4d5e6f708192a3b4c5";

#[derive(Debug)]
pub struct Owner {
    pub pgp: Pgp,
    pub user: PrivateKey,
    pub address: PrivateKey,
}

#[derive(Debug)]
pub struct NodeKey {
    pub key: PrivateKey,
    pub passphrase: String,
    pub armored_key: String,
    pub armored_passphrase: String,
    pub armored_signature: String,
}

#[derive(Debug)]
pub struct HashKey {
    pub secret: Vec<u8>,
    pub armored: String,
}

impl Owner {
    #[must_use]
    pub fn new() -> Owner {
        Owner::with_address(new_key(&Pgp::new(), EMAIL))
    }

    #[must_use]
    pub fn with_address(address: PrivateKey) -> Owner {
        let pgp = Pgp::new();
        Owner {
            user: new_key(&pgp, EMAIL),
            address,
            pgp,
        }
    }

    #[must_use]
    pub fn routes(&self, request: &Request) -> Option<ResponseFuture> {
        match request.route() {
            ("GET", "/core/v4/users") => Some(json_response(200, &self.user())),
            ("GET", "/core/v4/addresses") => Some(json_response(200, &self.addresses())),
            _ => None,
        }
    }

    fn user(&self) -> Value {
        json!({
            "Code": 1000,
            "User": {
                "ID": "user-value",
                "Name": "tester",
                "Keys": [{
                    "ID": "user-key",
                    "Version": 3,
                    "PrivateKey": lock_key(&self.user, PASSPHRASE),
                    "Primary": 1,
                    "Active": 1,
                }],
            },
        })
    }

    fn addresses(&self) -> Value {
        let token = self
            .pgp
            .encrypt()
            .to(self.user.public_key())
            .armored_message(TOKEN)
            .unwrap();
        let signature = self
            .pgp
            .sign()
            .with_key(self.user.clone())
            .armored_detached(TOKEN)
            .unwrap();

        json!({
            "Code": 1000,
            "Total": 1,
            "Addresses": [{
                "ID": ADDRESS_ID,
                "Email": EMAIL,
                "DisplayName": "Tester",
                "Send": 1,
                "Receive": 1,
                "Status": 1,
                "Type": 1,
                "Order": 1,
                "HasKeys": 1,
                "Keys": [{
                    "ID": ADDRESS_KEY_ID,
                    "Version": 3,
                    "Flags": 3,
                    "PrivateKey": lock_key(&self.address, TOKEN),
                    "Token": token,
                    "Signature": signature,
                    "Primary": 1,
                    "Active": 1,
                }],
            }],
        })
    }

    #[must_use]
    pub fn node_key(&self, parent: &PublicKey) -> NodeKey {
        let passphrase = rikka_proton_pgp::random_hex(16);
        let key = new_key(&self.pgp, EMAIL);

        NodeKey {
            armored_key: lock_key(&key, &passphrase),
            armored_passphrase: self
                .pgp
                .encrypt()
                .to(parent.clone())
                .armored_message(&passphrase)
                .unwrap(),
            armored_signature: self
                .pgp
                .sign()
                .with_key(self.address.clone())
                .armored_detached(&passphrase)
                .unwrap(),
            passphrase,
            key,
        }
    }

    #[must_use]
    pub fn name(&self, parent: &PublicKey, name: &str) -> String {
        self.pgp
            .encrypt()
            .to(parent.clone())
            .signed_by(self.address.clone())
            .armored_message(name)
            .unwrap()
    }

    #[must_use]
    pub fn hash_key(&self, node: &PrivateKey) -> HashKey {
        let secret = rikka_proton_pgp::random_hex(32).into_bytes();
        let armored = self
            .pgp
            .encrypt()
            .to(node.public_key())
            .signed_by(node.clone())
            .armored_message(&secret)
            .unwrap();

        HashKey { secret, armored }
    }
}

impl Default for Owner {
    fn default() -> Owner {
        Owner::new()
    }
}

#[must_use]
pub fn new_key(pgp: &Pgp, email: &str) -> PrivateKey {
    pgp.generate_key()
        .user_id("tester", email)
        .generate()
        .unwrap()
}

#[must_use]
pub fn lock_key(key: &PrivateKey, secret: impl AsRef<[u8]>) -> String {
    key.lock(secret).unwrap().export_armored().unwrap()
}
