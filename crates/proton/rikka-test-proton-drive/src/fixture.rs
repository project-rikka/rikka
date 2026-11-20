use rikka_proton_pgp::{Pgp, PrivateKey, SessionKey, SignatureContext, unarmor};
use rikka_test_proton::{ADDRESS_ID, EMAIL, HashKey, NodeKey, Owner, new_key};
use serde_json::{Value, json};

use crate::ids::{FOREIGN_OWNER, ROOT, SHARE, VOLUME};

const INVITER_CONTEXT: &str = "drive.share-member.inviter";
pub(crate) const NOW: i64 = 1_700_000_000;

pub(crate) struct Fixture {
    pub(crate) pgp: Pgp,
    pub(crate) owner: Owner,
    pub(crate) share: NodeKey,
    pub(crate) root: NodeKey,
    pub(crate) root_hash: HashKey,
    pub(crate) folder: NodeKey,
    pub(crate) folder_hash: HashKey,
    pub(crate) file: NodeKey,
    pub(crate) content: SessionKey,
    pub(crate) content_packet: String,
    pub(crate) content_signature: String,
    pub(crate) verification: [u8; 32],
    pub(crate) invitee_key: PrivateKey,
    pub(crate) foreign_key: PrivateKey,
}

impl Fixture {
    pub(crate) fn new() -> Fixture {
        let owner = Owner::new();
        let pgp = owner.pgp;
        let share = owner.node_key(&owner.address.public_key());
        let root = owner.node_key(&share.key.public_key());
        let root_hash = owner.hash_key(&root.key);
        let folder = owner.node_key(&root.key.public_key());
        let folder_hash = owner.hash_key(&folder.key);
        let file = owner.node_key(&root.key.public_key());
        let content = pgp
            .encrypt()
            .to(file.key.public_key())
            .session_key()
            .unwrap();
        let content_packet = rikka_proton_api::base64::encode(
            pgp.encrypt()
                .to(file.key.public_key())
                .key_packets(&content)
                .unwrap(),
        );
        let content_signature = pgp
            .sign()
            .with_key(file.key.clone())
            .armored_detached(content.as_bytes())
            .unwrap();

        Fixture {
            pgp,
            invitee_key: new_key(&pgp, EMAIL),
            foreign_key: new_key(&pgp, EMAIL),
            owner,
            share,
            root,
            root_hash,
            folder,
            folder_hash,
            file,
            content,
            content_packet,
            content_signature,
            verification: rikka_proton_pgp::random_bytes::<32>(),
        }
    }

    pub(crate) fn root_name(&self, name: &str) -> String {
        self.owner.name(&self.root.key.public_key(), name)
    }

    pub(crate) fn share_session_key(&self) -> SessionKey {
        let packets = unarmor(&self.share.armored_passphrase).unwrap();
        self.pgp
            .decrypt()
            .with_key(self.owner.address.clone())
            .session_key(&packets)
            .unwrap()
    }

    pub(crate) fn invitation_packet(
        &self,
        to: &rikka_proton_pgp::PublicKey,
        signer: &PrivateKey,
    ) -> (String, String) {
        let packet = self
            .pgp
            .encrypt()
            .to(to.clone())
            .key_packets(&self.share_session_key())
            .unwrap();
        let signature = self
            .pgp
            .sign()
            .with_key(signer.clone())
            .with_context(SignatureContext::new(INVITER_CONTEXT).critical(true))
            .detached(&packet)
            .unwrap();

        (
            rikka_proton_api::base64::encode(packet),
            rikka_proton_api::base64::encode(signature),
        )
    }

    pub(crate) fn sealed_block(&self, plain: &[u8]) -> (Vec<u8>, String) {
        let data = self
            .pgp
            .encrypt()
            .with_session_key(self.content.clone())
            .message(plain)
            .unwrap();
        let signature = self
            .pgp
            .sign()
            .with_key(self.owner.address.clone())
            .detached(plain)
            .unwrap();
        let armored = self
            .pgp
            .encrypt()
            .to(self.file.key.public_key())
            .with_session_key(self.content.clone())
            .armored_message(signature)
            .unwrap();
        (data, armored)
    }

    pub(crate) fn sealed_thumbnail(&self, plain: &[u8]) -> Vec<u8> {
        self.pgp
            .encrypt()
            .with_session_key(self.content.clone())
            .signed_by(self.owner.address.clone())
            .message(plain)
            .unwrap()
    }

    pub(crate) fn attributes(&self, sent: &str) -> String {
        self.pgp
            .encrypt()
            .to(self.file.key.public_key())
            .signed_by(self.owner.address.clone())
            .armored_message(sent)
            .unwrap()
    }

    pub(crate) fn address_signature(&self, data: impl AsRef<[u8]>) -> String {
        self.pgp
            .sign()
            .with_key(self.owner.address.clone())
            .armored_detached(data)
            .unwrap()
    }

    pub(crate) fn owned_share_json(&self, kind: i64) -> Value {
        json!({
            "Code": 1000,
            "ShareID": SHARE,
            "Type": kind,
            "State": 1,
            "LinkID": ROOT,
            "VolumeID": VOLUME,
            "VolumeType": 1,
            "Creator": EMAIL,
            "Flags": 1,
            "Locked": false,
            "Key": self.share.armored_key,
            "Passphrase": self.share.armored_passphrase,
            "PassphraseSignature": self.share.armored_signature,
            "AddressID": ADDRESS_ID,
            "CreationTime": NOW,
            "Memberships": [],
        })
    }

    pub(crate) fn member_share_json(&self) -> Value {
        let session_key = self
            .pgp
            .encrypt()
            .to(self.foreign_key.public_key())
            .session_key()
            .unwrap();
        let passphrase = self
            .pgp
            .encrypt()
            .to(self.foreign_key.public_key())
            .with_session_key(session_key.clone())
            .armored_message(&self.share.passphrase)
            .unwrap();
        let signature = self
            .pgp
            .sign()
            .with_key(self.foreign_key.clone())
            .armored_detached(&self.share.passphrase)
            .unwrap();
        let packet = self
            .pgp
            .encrypt()
            .to(self.owner.address.public_key())
            .key_packets(&session_key)
            .unwrap();

        json!({
            "Code": 1000,
            "ShareID": SHARE,
            "Type": 2,
            "State": 1,
            "LinkID": ROOT,
            "VolumeID": VOLUME,
            "Creator": FOREIGN_OWNER,
            "Flags": 0,
            "Locked": null,
            "Key": self.share.armored_key,
            "Passphrase": passphrase,
            "PassphraseSignature": signature,
            "AddressID": null,
            "CreationTime": NOW,
            "EditorsCanShare": true,
            "Memberships": [{
                "MemberID": "membership",
                "ShareID": SHARE,
                "AddressID": ADDRESS_ID,
                "AddressKeyID": "address-key",
                "Inviter": FOREIGN_OWNER,
                "Permissions": 6,
                "KeyPacket": rikka_proton_api::base64::encode(packet),
                "KeyPacketSignature": null,
                "SessionKeySignature": null,
                "State": 1,
                "CreateTime": NOW,
            }],
        })
    }
}
